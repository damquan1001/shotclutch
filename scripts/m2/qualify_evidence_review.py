"""Qualify evidence-based review locally, using real media frame boundaries."""
import argparse,csv,json,math,os,subprocess,sys,time,hashlib
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
from qualify_retake_joins import native_app,native_ranges,read,TICKS,coverage,digest,remove_span
def call_retry(command,name,args=None):
    for _ in range(12):
        try:return command(name,args)
        except RuntimeError as error:
            if str(error)!="timeout":raise
            if name=="file.open":return None
    raise TimeoutError(name)

def keeper_range(path):
    span=read(path).get("source_seconds")
    if not isinstance(span,list) or len(span)!=2 or any(type(t) not in (int,float) or not math.isfinite(t) for t in span) or not 0<=span[0]<span[1]:
        raise ValueError("Keeper label requires two finite increasing source_seconds")
    return tuple(round(t*TICKS) for t in span)
def editorial_input(template,out):
    import copy
    cases=json.loads((Path(__file__).resolve().parents[2]/"crates/engine/test-data/take-review-editorial.json").read_text())
    data=copy.deepcopy(template);words=[];positions=[]
    for number,case in enumerate(cases):
        first=len(words);at=number*120000
        for section in ("earlier","later"):
            for text in case[section].split():
                words.append(dict(text=text,start=at*TICKS//1000,end=(at+180)*TICKS//1000));at+=250
            if section=="earlier":middle=len(words)
            at+=1000
        positions.append(dict(case,first=first,middle=middle,end=len(words)))
    analysis=data["analysis"];end=(at+1000)*TICKS//1000;analysis["source"]["range"]=dict(start=0,duration=end)
    analysis["chunks"]=analysis["chunks"][:1];chunk=analysis["chunks"][0];chunk.update(coverage=dict(start=0,duration=end),quiet=[],edge_quiet=[],peaks=[]);chunk["transcript"]["words"]=words
    analysis["retake_recheck"]=None
    data["map"].update(candidates=[],takes=[],automatic=[],overrides=[],conflicts=[],corrections={})
    data["map"]["cleanup"].update(takeMatching="orderedV6",cutFillers=False,cutPauses=False)
    path=out/"editorial-input.json";path.write_text(json.dumps(data,separators=(",",":")),encoding="utf8")
    return path,positions
def editorial_score(result,cases):
    groups={g["id"]:g for g in result["request"]["groups"]};rows=[]
    for case in cases:
        related=[g for g in groups.values() if any(s["first"]>=case["first"] and s["first"]<case["middle"] for s in g["attempts"]) and any(s["first"]>=case["middle"] and s["end"]<=case["end"] for s in g["attempts"])]
        removals=[]
        for r in result["report"]["decisions"]:
            if r["id"] not in {g["id"] for g in related}:continue
            v=r.get("verification") or {};d=r["decision"]
            if d["kind"] in ("replace_earlier","remove_earlier_fragment") and all(v.get(k) for k in ("approve","information_preserved","meaning_unchanged","sentence_intact")):removals.append(d["discard"])
        prediction="remove" if removals else "keep"
        rows.append(dict(id=case["id"],split=case["split"],expected=case["expected"],prediction=prediction,retrieved=bool(related),correct=prediction==case["expected"]))
    return rows
def main():
    p=argparse.ArgumentParser()
    p.add_argument("--exe",type=Path,required=True);p.add_argument("--worker",type=Path,required=True)
    p.add_argument("--project",type=Path,required=True);p.add_argument("--labels-root",type=Path,required=True)
    p.add_argument("--models-dir",type=Path,required=True);p.add_argument("--server",type=Path,required=True)
    p.add_argument("--keeper-label",type=Path,help="Private JSON source_seconds range; defaults to labels-root/surviving-pairs/keeper-label.json")
    p.add_argument("--output",type=Path,required=True);args=p.parse_args()
    out=args.output.resolve();out.mkdir(parents=True,exist_ok=True);original_hash=digest(args.project)
    labels={r["id"]:r["verdict"] for r in csv.DictReader((args.labels_root/"remaining-retake-review/labels.csv").open(encoding="utf-8-sig")) if r["human_verified"]=="yes"}
    keeps=[tuple(round(t*TICKS) for t in c["range"]) for c in read(args.labels_root/"remaining-retake-review/report.json")["cases"] if labels.get(c["id"])=="keep"]
    bad=[tuple(c["range"]) for c in read(args.labels_root/"remaining-retakes-reviewed/report.json")["changes"] if labels.get(c["id"]) in ("bad_take","bad_take_partial")]
    pair=read(args.labels_root/"surviving-pairs/report.json")["removed_source_seconds"];pair=tuple(round(t*TICKS) for t in pair)
    glasses_keeper=keeper_range(args.keeper_label or args.labels_root/"surviving-pairs/keeper-label.json")
    def score(ranges,length):
        kept=sum(coverage(s,ranges) for s in keeps);remaining=sum(coverage(s,ranges) for s in bad)
        details=[]
        for kind,spans in (("keep",keeps),("bad_take",bad)):
            for lo,hi in spans:
                retained=[(max(lo,a),min(hi,b)) for a,b in ranges if max(lo,a)<min(hi,b)]
                missing=[(lo,hi)]
                for a,b in retained:
                    missing=remove_span(missing,a,b)
                discrepancy=missing if kind=="keep" else retained
                details.append(dict(kind=kind,source_seconds=[lo/TICKS,hi/TICKS],discrepancy_seconds=[[a/TICKS,b/TICKS] for a,b in discrepancy],passed=not discrepancy))
        return dict(duration_seconds=length/TICKS,keep_seconds=kept/TICKS,missing_keep_seconds=(sum(b-a for a,b in keeps)-kept)/TICKS,bad_take_seconds_remaining=remaining/TICKS,glasses_restart_remaining=coverage(pair,ranges)/TICKS,glasses_keeper_missing=((glasses_keeper[1]-glasses_keeper[0])-coverage(glasses_keeper,ranges))/TICKS,label_details=details,passed_labels=kept==sum(b-a for a,b in keeps) and remaining==0 and coverage(pair,ranges)==0 and coverage(glasses_keeper,ranges)==glasses_keeper[1]-glasses_keeper[0])
    summary=dict(project_sha256=original_hash,manual_overrides_used=False,models={},baselines={})
    try:summary["gpu"]=subprocess.run(["nvidia-smi","--query-gpu=name,memory.total","--format=csv,noheader"],capture_output=True,text=True,timeout=5,creationflags=0x08000000).stdout.strip()
    except (OSError,subprocess.TimeoutExpired):summary["gpu"]="unavailable"
    with native_app(args.exe,out) as command:
        print("Opening native project",flush=True)
        call_retry(command,"file.open",{"path":str(args.project.resolve())})
        document=read(args.project)["project"]["preedit"];old=max(document["maps"],key=int);opts=document["maps"][old]["cleanup"].copy();del document
        state=call_retry(command,"preedit.preview",{"map":old})
        bare=command("preedit.review",{"map":old,"expectedRevision":state["revision"],"action":{"kind":"resetDecisions"}})["map"]
        for variant in ("orderedV6","orderedV7","orderedV8"):
            state=call_retry(command,"preedit.preview",{"map":bare})
            key=command("preedit.review",{"map":bare,"expectedRevision":state["revision"],"action":{"kind":"cleanUp","options":dict(opts,takeMatching=variant)}})["map"]
            ranges,duration=native_ranges(command,key);summary["baselines"][variant]=score(ranges,duration)
            print("Baseline",variant,summary["baselines"][variant],flush=True)
            # Use current head as the parent for all later branches.
            bare=key
        for model in ("granite-retake-2b","qwen3-take-8b"):
            state=call_retry(command,"preedit.preview",{"map":bare})
            base=command("preedit.review",{"map":bare,"expectedRevision":state["revision"],"action":{"kind":"cleanUp","options":dict(opts,takeMatching="orderedV6")}})["map"]
            baseline_segments,baseline_duration=native_ranges(command,base)
            data=command("preedit.takeReviewInput",{"map":base})
            print(model,len(data["request"]["groups"]),"candidate groups",flush=True)
            group_ranges=[(span["first"],span["end"]) for g in data["request"]["groups"] for span in g["attempts"]]
            # Retrieval coverage is speech-word coverage, separate from source-range cut accuracy.
            words=[w for c in data["analysis"]["chunks"] if c.get("transcript") for w in c["transcript"]["words"]]
            covered=set(i for lo,hi in group_ranges for i in range(lo,hi))
            total_bad_words=sum(coverage((w["start"],w["end"]),bad) for w in words)/TICKS
            retrieval_bad=sum(coverage((words[i]["start"],words[i]["end"]),bad) for i in covered)/TICKS
            input_path=out/f"{model}-input.json";answer=out/f"{model}-answer.json";input_path.write_text(json.dumps(data,ensure_ascii=False,separators=(",",":")),encoding="utf8");fixture_path,fixture_cases=editorial_input(data,out);del data
            environment=dict(os.environ,SHOTCLUTCH_DATA_DIR=str(args.models_dir.resolve().parent),SHOTCLUTCH_LLAMA_SERVER=str(args.server.resolve()))
            reuse=answer.exists() and read(answer).get("request")==read(input_path)["request"]
            if reuse:
                cached=read(answer);cached["source_stamp"]=read(input_path)["source_stamp"];answer.write_text(json.dumps(cached),encoding="utf8")
                peak_vram=0
                print(model,"reusing completed decisions for identical checkpoint",flush=True)
            else:
                with (out/f"{model}.log").open("wb") as log:
                    child=subprocess.Popen([str(args.worker.resolve()),str(input_path),str(answer),model],env=environment,stdout=log,stderr=log)
                    last=None;peak_vram=0
                    while child.poll() is None:
                        time.sleep(10)
                        call_retry(command,"preedit.preview",{"map":base})
                        try:
                            query=subprocess.run(["nvidia-smi","--query-gpu=memory.used","--format=csv,noheader,nounits"],capture_output=True,text=True,timeout=5,creationflags=0x08000000)
                            peak_vram=max(peak_vram,max(int(v) for v in query.stdout.splitlines()))
                        except (OSError,ValueError,subprocess.TimeoutExpired):pass
                        progress=answer.with_suffix(".progress.json")
                        if progress.exists():
                            try:status=read(progress)
                            except (OSError,json.JSONDecodeError):continue
                            now=status.get("completed",0)
                            if last is None or now>=last+10:
                                print(model,status,flush=True);last=now
                    if child.returncode:raise RuntimeError(f"{model} failed; inspect {log.name}")
            result=read(answer)
            state=call_retry(command,"preedit.preview",{"map":base})
            applied=command("preedit.review",{"map":base,"expectedRevision":state["revision"],"action":{"kind":"evidenceReview","response":result}})
            key=applied["map"]
            ranges,duration=native_ranges(command,key);stats=score(ranges,duration)
            stats["review_summary"]=applied.get("reviewSummary")
            print(model,"frame-aligned result",{k:v for k,v in stats.items() if k!="label_details"},flush=True)
            # Compare the actual native segments, including the final frame boundaries.
            def difference_seconds(left,right):
                remaining=left[:]
                for a,b in right:remaining=remove_span(remaining,a,b)
                return sum(b-a for a,b in remaining)/TICKS
            stats["newly_removed_seconds"]=difference_seconds(baseline_segments,ranges)
            stats["restored_seconds"]=difference_seconds(ranges,baseline_segments)
            stats.update(retrieval_bad_word_total_seconds=total_bad_words,retrieval_bad_word_fraction=(retrieval_bad/total_bad_words if total_bad_words else None),reused_complete_answer=reuse,peak_system_gpu_memory_mib=peak_vram,retrieval_bad_speech_seconds=retrieval_bad,groups=result["report"]["total_groups"],completed_groups=result["report"]["completed_groups"],failures=len(result["report"]["failures"]),elapsed_seconds=result["report"]["elapsed_seconds"],peak_process_memory_bytes=result["report"]["peak_memory_bytes"],new_map=key)
            summary["models"][model]=stats
            # Equal scores prefer the smaller reviewer.
            summary["selected_model"]=min(summary["models"],key=lambda k:(sum(not row["passed"] for row in summary["models"][k]["label_details"])+summary["models"][k].get("held_out_errors",0),0 if k=="granite-retake-2b" else 1))
            # Test checkpoint undo and redo through the real command registry.
            command("edit.undo")
            assert native_ranges(command,base)==(baseline_segments,baseline_duration)
            try:command("preedit.preview",{"map":key})
            except RuntimeError as error:
                if "not_found" not in str(error).lower():raise
            else:raise AssertionError("Undo left the reviewed map installed")
            command("edit.redo")
            stats["undo_verified"]=True
            (out/"report-in-progress.json").write_text(json.dumps(summary,indent=2))
            print(model,"saving verified edit",flush=True)
            saved=out/f"Smart Cut - {model}.scproj"
            # Save As deliberately uses exclusive creation. Retain any older artifact
            # under a distinct name before publishing a requalified project.
            if saved.exists():saved.rename(out/f"Smart Cut - {model} - prior-{time.time_ns()}.scproj")
            try:command("file.saveAs",{"path":str(saved)})
            except RuntimeError as error:
                if str(error)!="timeout":raise
                limit=time.monotonic()+900
                while not saved.exists():
                    if time.monotonic()>limit:raise TimeoutError("save did not publish")
                    time.sleep(2)
                    try:command("preedit.preview",{"map":key})
                    except RuntimeError as error:
                        if str(error)!="timeout":raise
            call_retry(command,"file.open",{"path":str(saved)})
            call_retry(command,"preedit.preview",{"map":key})
            assert native_ranges(command,key)==(ranges,duration)
            stats["reopened"]=True
            command("edit.undo")
            assert native_ranges(command,base)==(baseline_segments,baseline_duration)
            try:command("preedit.preview",{"map":key})
            except RuntimeError as error:
                if "not_found" not in str(error).lower():raise
            else:raise AssertionError("Reopened Undo left the reviewed map installed")
            command("edit.redo")
            assert native_ranges(command,key)==(ranges,duration)
            stats["reopened_undo_verified"]=True
            fixture_answer=out/f"{model}-editorial-answer.json"
            with (out/f"{model}-editorial.log").open("wb") as log:
                child=subprocess.Popen([str(args.worker.resolve()),str(fixture_path),str(fixture_answer),model],env=environment,stdout=log,stderr=log)
                while child.poll() is None:
                    time.sleep(10);call_retry(command,"preedit.preview",{"map":key})
                if child.returncode:raise RuntimeError("Editorial qualification worker failed")
            stats["editorial_cases"]=editorial_score(read(fixture_answer),fixture_cases)
            stats["held_out_errors"]=sum(not r["correct"] for r in stats["editorial_cases"] if r["split"]=="held_out")
            stats["authored_errors"]=sum(not r["correct"] for r in stats["editorial_cases"])
            bare=key
            (out/"report.json").write_text(json.dumps(summary,indent=2))
    assert digest(args.project)==original_hash
    summary["original_project_unchanged"]=True
    summary["unseen_recording_accuracy"]="unqualified: only one previously inspected recording is available"
    summary["selected_model"]=min(summary["models"],key=lambda k:(sum(not row["passed"] for row in summary["models"][k]["label_details"])+summary["models"][k]["authored_errors"],0 if k=="granite-retake-2b" else 1))
    summary["promote"]=summary["models"][summary["selected_model"]]["passed_labels"] and summary["models"][summary["selected_model"]]["authored_errors"]==0
    (out/"report.json").write_text(json.dumps(summary,indent=2));print(json.dumps(summary,indent=2),flush=True)
if __name__=="__main__":main()
