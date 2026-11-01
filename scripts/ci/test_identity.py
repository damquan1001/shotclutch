"""Branding guard permits compatibility/legal facts but rejects active product names."""
import importlib.util
from pathlib import Path
import unittest
spec=importlib.util.spec_from_file_location("identity_guard",Path(__file__).resolve().parents[1]/"check-identity.py")
guard=importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)
old="film"+"craft"
class IdentityGuardTests(unittest.TestCase):
    def test_binary_shipping_assets_cannot_hide_legacy_names(self):
        for encoding in ['ascii', 'utf-16-le', 'utf-16-be']:
            self.assertTrue(guard.binary_branding('packaging/macos/dmg/dmg-layout.DS_Store', old.encode(encoding)))
        self.assertFalse(guard.binary_branding('docs/m0/evidence/archive.bin', old.encode()))
        self.assertFalse(guard.binary_branding('apps/shotclutch-web/web/index.html', old.encode()))
        self.assertFalse(guard.binary_branding('assets/app-icon/shotclutch.png', b'original geometric artwork'))
    def test_product_and_cargo_names_are_rejected(self):
        self.assertIsNone(guard.category("apps/shotclutch/src/new.rs",old+" editor"))
        self.assertIsNone(guard.category("Cargo.toml",'name = "'+old+'-editor"'))
    def test_explicit_alias_is_allowed_only_in_its_implementation(self):
        line='pub const OLD: &str = "'+old+'.project";'
        self.assertTrue(guard.category("crates/format/src/lib.rs",line))
        self.assertIsNone(guard.category("apps/shotclutch/src/new.rs",line))
    def test_copyright_and_historical_data_remain_truthful(self):
        self.assertTrue(guard.category("LICENSE-MIT",old+" contributors"))
        self.assertTrue(guard.category("docs/m0/evidence/old.json",old))
        self.assertTrue(guard.category("assets/example.png.attribution",'author: '+old+' contributors'))
if __name__ == "__main__":
    unittest.main()
