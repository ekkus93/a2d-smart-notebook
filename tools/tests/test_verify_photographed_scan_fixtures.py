from __future__ import annotations

import hashlib
import tempfile
import unittest
from pathlib import Path

from tools import verify_photographed_scan_fixtures as verifier


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


class PhotographedFixtureVerifierTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def fixture(
        self,
        fixture_id: str,
        physical_sheet_id: str,
        content_revision_id: str,
    ) -> dict[str, object]:
        raw_bytes = f"raw-{fixture_id}".encode()
        normalized_bytes = f"normalized-{fixture_id}".encode()
        raw_path = Path("raw") / f"{fixture_id}.jpg"
        normalized_path = Path("normalized") / f"{fixture_id}.png"
        (self.root / raw_path).parent.mkdir(parents=True, exist_ok=True)
        (self.root / normalized_path).parent.mkdir(parents=True, exist_ok=True)
        (self.root / raw_path).write_bytes(raw_bytes)
        (self.root / normalized_path).write_bytes(normalized_bytes)
        return {
            "id": fixture_id,
            "photographed": True,
            "raw_capture_path": raw_path.as_posix(),
            "raw_capture_format": "jpeg",
            "raw_capture_byte_length": len(raw_bytes),
            "raw_capture_sha256": digest(raw_bytes),
            "normalized_ocr_path": normalized_path.as_posix(),
            "normalized_ocr_format": "png",
            "normalized_ocr_byte_length": len(normalized_bytes),
            "normalized_ocr_sha256": digest(normalized_bytes),
            "normalized_rotation_degrees": 0,
            "pipeline_version": 1,
            "source": {
                "description": "Test fixture",
                "consent": "Explicit test consent",
                "license": "Apache-2.0",
                "attribution": "A2D tests",
            },
            "device": {
                "manufacturer": "Example",
                "model": "Test Phone",
                "android_version": "16",
                "build_fingerprint": "example/test/device:16/build",
            },
            "camera": {
                "camera_id": "0",
                "lens_facing": "back",
                "width_px": 4000,
                "height_px": 3000,
            },
            "conditions": {
                "captured_at_utc": "2026-07-29T00:00:00Z",
                "lighting": "office LED",
                "capture_angle": "near perpendicular",
                "stabilization": "handheld",
                "distance_cm": 35,
                "notes": "Synthetic metadata for verifier tests only",
            },
            "page": {
                "page_identity": "A2D:TEST:PAGE:42",
                "physical_sheet_id": physical_sheet_id,
                "content_revision_id": content_revision_id,
                "print_source": "test printer",
                "paper": "US Letter plain paper",
                "writing_instrument": "black ballpoint",
            },
        }

    def pair(
        self,
        relation: str,
        baseline_id: str = "baseline",
        candidate_id: str = "candidate",
    ) -> dict[str, str]:
        return {
            "id": f"{baseline_id}-to-{candidate_id}",
            "baseline_fixture_id": baseline_id,
            "candidate_fixture_id": candidate_id,
            "expected_relation": relation,
            "labeling_notes": "Known test relationship",
        }

    def test_complete_fixture_and_near_duplicate_pair_are_accepted(self) -> None:
        baseline = self.fixture("baseline", "sheet-1", "revision-1")
        candidate = self.fixture("candidate", "sheet-1", "revision-1")
        fixtures = {
            "baseline": verifier.verify_fixture(self.root, baseline),
            "candidate": verifier.verify_fixture(self.root, candidate),
        }

        pair_id = verifier.verify_pair(self.pair("near_duplicate"), fixtures)

        self.assertEqual(pair_id, "baseline-to-candidate")

    def test_digest_drift_is_rejected(self) -> None:
        fixture = self.fixture("baseline", "sheet-1", "revision-1")
        fixture["normalized_ocr_sha256"] = "0" * 64

        with self.assertRaisesRegex(SystemExit, "SHA-256 drift"):
            verifier.verify_fixture(self.root, fixture)

    def test_revision_requires_same_sheet_and_different_revision(self) -> None:
        baseline = self.fixture("baseline", "sheet-1", "revision-1")
        candidate = self.fixture("candidate", "sheet-2", "revision-2")
        fixtures = {
            "baseline": verifier.verify_fixture(self.root, baseline),
            "candidate": verifier.verify_fixture(self.root, candidate),
        }

        with self.assertRaisesRegex(SystemExit, "revision requires one physical sheet"):
            verifier.verify_pair(self.pair("revision"), fixtures)

    def test_calibration_input_is_create_new_and_complete(self) -> None:
        baseline = self.fixture("baseline", "sheet-1", "revision-1")
        candidate = self.fixture("candidate", "sheet-1", "revision-2")
        fixtures = {"baseline": baseline, "candidate": candidate}
        pairs = [self.pair("revision")]
        output = self.root / "reports" / "input.tsv"

        verifier.write_calibration_input(output, fixtures, pairs)
        lines = output.read_text(encoding="utf-8").splitlines()

        self.assertEqual(lines[0] + "\n", verifier.CALIBRATION_INPUT_HEADER)
        self.assertEqual(
            lines[1].split("\t"),
            [
                "baseline-to-candidate",
                "revision",
                "baseline",
                "normalized/baseline.png",
                "1",
                "candidate",
                "normalized/candidate.png",
                "1",
            ],
        )
        with self.assertRaises(FileExistsError):
            verifier.write_calibration_input(output, fixtures, pairs)


if __name__ == "__main__":
    unittest.main()
