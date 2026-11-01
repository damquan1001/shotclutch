"""Fail active branding regressions; enumerate every retained legacy-name reference by reason."""
import argparse,fnmatch,json,re,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
WORDS=re.compile('film'+'craft|art'+'craft|fc'+'proj',re.I)
HISTORY=('docs/m0/','docs/filmcraft_original_docs/','crates/format/tests/fixtures/')
COMPAT={
 'packaging/linux/package.sh':'Inherited maintainer metadata environment override',
 'scripts/ci/package-smoke.py':'Keep both inherited and canonical media tool overrides out of standalone execution',
 'apps/shotclutch/build.rs':'Inherited Windows resource requirement environment alias',
 'packaging/windows/package.ps1':'Inherited packaging environment aliases',
 'crates/engine/src/presets.rs':'Read-only inherited effect-preset identifier',
 'crates/engine/src/export_tools.rs':'Read-only inherited export-preset identifier',
 'crates/project/src/gtemplate.rs':'Read-only inherited graphics-template identifier',
 'crates/interchange/src/otio.rs':'Read-only inherited interchange metadata namespace',
 'scripts/m0/validate-package.ps1':'Negative check that installers do not claim the legacy extension',
 'apps/shotclutch-cli/tests/it/cli.rs':'End-to-end legacy read/save-as/source-preservation checks',
 'README.md':'Legacy project/environment documentation and archive navigation',
 'docs/SHOTCLUTCH_IDENTITY.md':'Explicit interface and format compatibility policy',
 'crates/env/src/lib.rs':'Central environment fallback and precedence tests',
 'crates/format/src/lib.rs':'Read-only format and filename aliases',
 'crates/format/src/autosave.rs':'Legacy autosave discovery without rewriting originals',
 'crates/engine/src/autosave.rs':'Legacy environment and recovery snapshot discovery',
 'crates/engine/src/autosave_tests.rs':'Original-preservation and recovery compatibility tests',
 'crates/engine/src/file_tests.rs':'Legacy import and source-preservation tests',
 'crates/format/tests/fixtures.rs':'Immutable historical fixture filenames',
 'apps/shotclutch/src/file_filters.rs':'Legacy extension case-filter regression tests',
 'crates/testkit/src/deadline.rs':'Inherited timeout control alias documented in its regression test',
 'crates/tts/src/kokoro/mod.rs':'Inherited voice-model environment override',
 'crates/tts-text/src/phonemize.rs':'Inherited dictionary test environment override',
 'crates/ui-egui/src/panels/settings.rs':'Inherited diagnostic environment override',
 'apps/shotclutch/src/args.rs':'Case-insensitive legacy project recognition tests',
 'apps/shotclutch-web/src/api.rs':'Thin browser API alias',
 'apps/shotclutch-web/web/index.html':'Thin browser diagnostic aliases',
 'apps/shotclutch-web/tests/smoke.mjs':'Browser alias object-identity assertion',
 'apps/shotclutch-web/src/recovery.rs':'Read-only legacy OPFS snapshot fallback',
 'apps/shotclutch-web/src/import.rs':'Legacy project picker extension',
 'apps/shotclutch/src/main.rs':'Open-dialog legacy extension; native saves remain new extension only',
 'crates/automation/src/server.rs':'Legacy resource read aliases and equivalence tests',
 'packaging/env.sh':'Inherited packaging environment fallbacks',
 'scripts/m0/benchmark.ps1':'Reproduction against the untouched original upstream checkout',
 'scripts/m0/desktop-smoke.py':'Negative legacy-token/port inputs test canonical precedence',
}
LEGACY_INPUT=re.compile(r'FILMCRAFT_|filmcraft[.](?:project|effect-presets|export-presets|graphicsTemplate)|fcproj|filmcraft://(?:document|commands)|window[.]filmcraft|["\x27]filmcraft["\x27]|filmcraft-ui-egui|filmcraft_original_docs',re.I)
def category(path,line):
    if path=='scripts/check-identity.py':return 'residual-check policy definitions'
    if path.startswith(HISTORY) or path in ['scripts/m0/prepare-upstream.ps1','contributors/contributors.json']:return 'historical provenance'
    if Path(path).name.startswith('LICENSE') or path in ['NOTICE','ATTRIBUTION.md','COPYRIGHT'] or path.endswith('.attribution'):return 'legal / retained-asset provenance'
    if path in ['apps/shotclutch/build.rs','packaging/macos/Info.plist.in'] and 'copyright' in line.lower():return 'required copyright notice'
    if re.search(r'https?://[^\s<>"`)]*(?:filmcraft|artcraft)',line,re.I):return 'genuine external reference'
    if path in COMPAT and LEGACY_INPUT.search(line):return 'compatibility: '+COMPAT[path]
    return None
def main():
    p=argparse.ArgumentParser();p.add_argument('--report',type=Path);a=p.parse_args()
    files=set(subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard','-z'],cwd=ROOT).decode().split('\0'))
    retained=[];errors=[]
    for name in sorted(files):
        path=ROOT/name
        if not name or not path.is_file():continue
        try:text=path.read_bytes().decode('utf-8')
        except UnicodeError:continue
        for number,line in enumerate(text.splitlines(),1):
            if not WORDS.search(line):continue
            reason=category(name,line)
            entry={'file':name,'line':number,'category':reason,'text':line}
            (retained if reason else errors).append(entry)
        if WORDS.search(name) and not name.startswith(HISTORY):errors.append({'file':name,'error':'legacy name in active filename'})
    report={'result':'passed' if not errors else 'failed','retained_references':retained,'unexpected':errors}
    if a.report:
        a.report.parent.mkdir(parents=True,exist_ok=True);a.report.write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({'result':report['result'],'retained_count':len(retained),'unexpected':errors},indent=2))
    raise SystemExit(bool(errors))

if __name__ == "__main__":
    main()
