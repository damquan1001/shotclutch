"""Score unassisted cleanup against existing source-time labels."""
import argparse,csv,json
from pathlib import Path
TICKS=254016000000
def union(ranges):
    out=[]
    for lo,hi in sorted(ranges):
        if out and out[-1][1]>=lo:out[-1][1]=max(hi,out[-1][1])
        else:out.append([lo,hi])
    return out
def seconds(a,b):
    return sum(max(0,min(y,v)-max(x,u)) for x,y in union(a) for u,v in union(b))/TICKS
def main():
    p=argparse.ArgumentParser();p.add_argument("ranges",type=Path);p.add_argument("labels_root",type=Path);p.add_argument("--output",type=Path,required=True);args=p.parse_args()
    load=lambda path:json.loads(path.read_text(encoding="utf-8-sig"))
    root=args.labels_root;labels={r["id"]:r["verdict"] for r in csv.DictReader((root/"remaining-retake-review/labels.csv").open(encoding="utf-8-sig")) if r["human_verified"]=="yes"}
    keeps=[tuple(round(t*TICKS) for t in c["range"]) for c in load(root/"remaining-retake-review/report.json")["cases"] if labels.get(c["id"])=="keep"]
    bad=[tuple(c["range"]) for c in load(root/"remaining-retakes-reviewed/report.json")["changes"] if labels.get(c["id"]) in ("bad_take","bad_take_partial")]
    runs=load(args.ranges);report={"keep_seconds":seconds(keeps,keeps),"bad_take_seconds":seconds(bad,bad),"manual_overrides_used":False,"model_run":False,"runs":{}}
    for name,data in runs.items():
        if not name.endswith("_automatic"):continue
        retained=data["retained"]
        report["runs"][name.removesuffix("_automatic")]={"duration":data["duration"]/TICKS,"keep_speech_retained":seconds(retained,keeps),"bad_take_speech_retained":seconds(retained,bad)}
    args.output.write_text(json.dumps(report,indent=2),encoding="utf8");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
