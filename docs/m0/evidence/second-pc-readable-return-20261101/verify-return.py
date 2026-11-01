"""Verify readable second-PC copies without modifying the project or source results."""
import argparse,hashlib,json,subprocess
from pathlib import Path
from fractions import Fraction
p=argparse.ArgumentParser();p.add_argument('--results',type=Path,required=True);p.add_argument('--cli',type=Path,required=True);p.add_argument('--manifest',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
a.output.mkdir(parents=True,exist_ok=True)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
profile=json.loads(next(a.results.glob('profile-*.json')).read_text(encoding='utf-8-sig'));manifest=json.loads(a.manifest.read_text(encoding='utf-8'))
assert all(x['sha256'].lower()==manifest['files']['packages/'+x['name']]['sha256'] for x in profile['packages'])
assert profile['fixture_sha256'].lower()==manifest['files']['fixture-1080p.mp4']['sha256']
commands=[]
def run(cmd,name):
 r=subprocess.run([str(x) for x in cmd],capture_output=True,timeout=120)
 (a.output/(name+'.stdout')).write_bytes(r.stdout);(a.output/(name+'.stderr.log')).write_bytes(r.stderr)
 commands.append({'command':[str(x) for x in cmd],'exit_code':r.returncode,'stdout':name+'.stdout','stderr':name+'.stderr.log'})
 assert r.returncode==0,r.stderr.decode(errors='replace')
 return r.stdout
project=json.loads((a.results/'Test.fcproj').read_text(encoding='utf-8-sig'))
assert project['format']=='filmcraft.project'
seqs=[]
for item in project['project']['items'].values():
 seq=item.get('kind',{}).get('Sequence')
 if not seq:continue
 settings=seq['settings'];assert (settings['width'],settings['height'])==(1920,1080)
 rate=Fraction(settings['frame_rate']['num'],settings['frame_rate']['den']);assert rate==30
 ends=[clip['start']+clip['duration'] for track in seq['video_tracks'] for clip in track['items']]
 frames=Fraction(max(ends),254016000000)*rate;assert frames==50
 seqs.append({'id':item['id'],'name':item['name'],'frames':int(frames),'width':1920,'height':1080,'fps':'30/1'})
assert seqs
cliout=json.loads(run([a.cli,'--data-dir',a.results/'cli-data','--project',a.results/'Test.fcproj','inspect','project'],'packaged-cli-project'))
probe=json.loads(run(['ffprobe','-v','error','-count_frames','-show_streams','-of','json',a.results/'Test.mov'],'ffprobe'))
v=next(s for s in probe['streams'] if s['codec_type']=='video');audio=next(s for s in probe['streams'] if s['codec_type']=='audio')
assert (v['width'],v['height'],v['avg_frame_rate'],int(v['nb_read_frames']))==(1920,1080,'30/1',50)
assert v['codec_name']=='prores' and int(audio['sample_rate'])==48000 and audio['channels']==2
run(['ffmpeg','-v','error','-xerror','-i',a.results/'Test.mov','-map','0:v:0','-f','null','-'],'video-decode')
pcm=run(['ffmpeg','-v','error','-xerror','-i',a.results/'Test.mov','-map','0:a:0','-f','f32le','-ac','2','-ar','48000','-'],'audio-decode')
assert len(pcm)//8==80000 and len(pcm)%8==0
# Do not retain decoded audio bytes in Git; its length/hash are sufficient.
(a.output/'audio-decode.stdout').unlink()
commands[-1]['stdout']='decoded PCM not retained; byte count/hash in report'
report={'result':'bounded_second_pc_core_workflow_pass','source_revision':manifest['source_revision'],'run_id':manifest['run_id'],'package_attempt':manifest['package_attempt'],'inputs':{name:{'bytes':(a.results/name).stat().st_size,'sha256':sha(a.results/name)} for name in ['Test.mov','Test.fcproj','observations.md','profile-20261101T091158763Z.json']},'verification_cli_sha256':sha(a.cli),'package_and_fixture_hashes':'match supplied kit','project':{'format':project['format'],'schema_version':project['schema_version'],'generator':project['generator'],'packaged_cli_load':'passed','sequences':seqs,'linked_media':'Original second-PC drive path preserved; no original edited or relinked by verification'},'video':{'codec':v['codec_name'],'width':1920,'height':1080,'fps':'30/1','frames':50,'full_decode':'passed'},'audio':{'codec':audio['codec_name'],'sample_rate':48000,'channels':2,'decoded_stereo_sample_frames':len(pcm)//8,'decoded_pcm_bytes':len(pcm),'decoded_pcm_sha256':hashlib.sha256(pcm).hexdigest()},'manual_workflow':'User reports portable launch, playback, trim/undo/redo, save/reopen, export, MSI lifecycle PASS; no external editing app needed','network_state':'connected (direct user clarification)','profile':profile,'commands':commands,'limitations':['Windows 10 Enterprise build 19045 on i5-4690 / RX 570 / approximately 24 GiB; not a second Windows 11 profile','No sustained-performance, working-set, dropped-frame or physical A/V measurements','No strict offline consumer-VM qualification','Earlier zero-filled copies superseded for output verification; their cause remains unknown']}
(a.output/'verification.json').write_bytes((json.dumps(report,indent=2)+'\n').encode())
print(json.dumps({'result':report['result'],'frames':50,'stereo_sample_frames':80000,'sequences':seqs}))
