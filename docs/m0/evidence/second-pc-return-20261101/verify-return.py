"""Read returned qualification artifacts; never modify original results."""
import argparse,hashlib,json,subprocess
from pathlib import Path
parser=argparse.ArgumentParser()
parser.add_argument('--results',type=Path,required=True)
parser.add_argument('--manifest',type=Path,required=True)
parser.add_argument('--output',type=Path,required=True)
args=parser.parse_args()
hash_bytes=lambda b:hashlib.sha256(b).hexdigest()
profiles=list(args.results.glob('profile-*.json'))
assert len(profiles)==1, 'Expected one profile'
profile=json.loads(profiles[0].read_text(encoding='utf-8-sig'))
manifest=json.loads(args.manifest.read_text(encoding='utf-8-sig'))
checks=[]
for item in profile['packages']:
    expected=manifest['files']['packages/'+item['name']]['sha256']
    checks.append({'name':item['name'],'expected_sha256':expected,'reported_sha256':item['sha256'].lower(),'matches':item['sha256'].lower()==expected})
fixture={'expected_sha256':manifest['files']['fixture-1080p.mp4']['sha256'],'reported_sha256':profile['fixture_sha256'].lower()}
fixture['matches']=fixture['expected_sha256']==fixture['reported_sha256']
files=[]
for name in ['ShotClutch Demo.fcproj','Test.mov']:
    b=(args.results/name).read_bytes()
    files.append({'name':name,'bytes':len(b),'sha256':hash_bytes(b),'nonzero_bytes':sum(v!=0 for v in b),'all_zero':bool(b) and not any(b)})
project={'result':'unreadable'}
try:
    data=json.loads((args.results/'ShotClutch Demo.fcproj').read_text(encoding='utf-8-sig'))
    project={'result':'json_parse_passed','keys':list(data)}
except (UnicodeError,ValueError) as e:
    project['error']=str(e)
probe=subprocess.run(['ffprobe','-v','error','-count_frames','-show_streams','-of','json',str(args.results/'Test.mov')],capture_output=True,timeout=90)
report={'source_revision':manifest['source_revision'],'run_id':manifest['run_id'],'package_attempt':manifest['package_attempt'],'input_folder':str(args.results),'profile_sha256':hash_bytes(profiles[0].read_bytes()),'observations_sha256':hash_bytes((args.results/'observations.md').read_bytes()),'package_hashes':checks,'fixture_hash':fixture,'file_integrity':files,'project_parse':project,'media_probe':{'exit_code':probe.returncode,'stderr':probe.stderr.decode('utf-8',errors='replace'),'result':json.loads(probe.stdout) if probe.returncode==0 else None},'manual_workflow':'User observation form reports PASS; accepted as manual evidence distinct from independent output verification','network_state':'Observation says PASS, connected/disconnected unknown; clarification requested','result':'needs_original_or_recopy' if any(f['all_zero'] for f in files) else 'requires_output_review','os_qualification_scope':'Observed Windows 10 Enterprise x64 build 19045; not Windows 11 second-profile qualification','limitations':['No independent project or export pass from zero-filled files','No measured dropped frames, memory, physical A/V drift or sustained playback qualification','Cause of zero-filled artifacts unknown; not attributed to product or transfer without originals']}
args.output.parent.mkdir(parents=True,exist_ok=True)
args.output.write_bytes((json.dumps(report,indent=2)+'\n').encode())
print(json.dumps({'result':report['result'],'package_hashes_match':all(c['matches'] for c in checks),'fixture_hash_matches':fixture['matches'],'files':files}))
