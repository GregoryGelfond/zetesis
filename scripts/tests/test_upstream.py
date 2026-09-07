"""Exact-source and bounded C++ decoder regressions for upstream fixtures."""
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch
import sys
import json
import os
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
SPEC = importlib.util.spec_from_file_location('upstream_assertions', ROOT / 'scripts/upstream_assertions.py')
UPSTREAM = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(UPSTREAM)
sys.path.insert(0, str(ROOT / 'scripts'))
RUN_SPEC = importlib.util.spec_from_file_location('compare_upstream', ROOT / 'scripts/compare-upstream.py')
RUN = importlib.util.module_from_spec(RUN_SPEC)
RUN_SPEC.loader.exec_module(RUN)


class UpstreamAssertions(unittest.TestCase):
    def test_empty_catalogue_and_unknown_native_contract_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            fixture = directory / 'cases.jsonl'
            fixture.write_text('')
            with self.assertRaisesRegex(ValueError, 'exactly 24'):
                UPSTREAM.verify_fixture(directory)
            # Native tags are checked independently of reference generation.
            cases = UPSTREAM.verify_fixture(ROOT / 'validation/upstream/clingo-5.8.2')
            cases[0]['native'] = 'typo'
            fixture.write_text(''.join(json.dumps(case) + '\n' for case in cases))
            os.symlink(ROOT / 'validation/upstream/clingo-5.8.2/originals', directory / 'originals')
            with self.assertRaisesRegex(ValueError, 'unknown native'):
                UPSTREAM.verify_fixture(directory)

    def test_report_cannot_alias_inputs_binaries_or_fixture_paths(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            protected = directory / 'binary'
            protected.write_text('preserve')
            linked = directory / 'linked'
            linked.symlink_to(protected)
            hard = directory / 'hard'
            os.link(protected, hard)
            fixture = directory / 'fixture'
            fixture.mkdir()
            for destination in [protected, linked, hard, fixture / 'new.json']:
                with self.subTest(path=destination), self.assertRaises(ValueError):
                    RUN.report_path(destination, [protected], fixture)
            RUN.report_path(directory / 'report.json', [protected], fixture)
            self.assertEqual(protected.read_text(), 'preserve')

    def test_expected_profile_refusal_does_not_accept_resource_or_load_errors(self):
        prefix = 'zetesis: source admission: '
        for reason in ['source bytes exceeded', 'could not load input', 'formula work limit']:
            completed = subprocess.CompletedProcess(['solver'], 2, '', prefix + reason + '\n')
            self.assertFalse(RUN.profile_refusal(completed))
        completed = subprocess.CompletedProcess(['solver'], 2, '',
            prefix + 'S0 does not admit conditional disjunction element\n')
        self.assertTrue(RUN.profile_refusal(completed))

    def test_all_selected_sources_and_metadata_reconstruct_exactly(self):
        cases = UPSTREAM.verify_fixture(ROOT / 'validation/upstream/clingo-5.8.2')
        self.assertEqual(len(cases), 24)
        self.assertEqual(sum(len(case['models']) for case in cases), 73)

    def test_cpp_literal_concatenation_retains_bytes(self):
        self.assertEqual(UPSTREAM.literal_sequence(r'"a.\n" /*x*/ "b(\"x\")."'), 'a.\nb("x").')

    def test_dynamic_strings_and_unsupported_escapes_are_refused(self):
        for source in ['prefix + "p."', 'R"(p.)"', r'"\x70."', r'"\u0070."']:
            with self.subTest(source=source), self.assertRaises(ValueError):
                UPSTREAM.literal_sequence(source)

    def test_balanced_arguments_ignore_string_and_comment_delimiters(self):
        text = '("p(1;2).", {"x,]"}, /* ) */ {1, 2})'
        end, commas = UPSTREAM.balanced(text, 0)
        self.assertEqual(end, len(text) - 1)
        self.assertEqual([text[index] for index in commas], [',', ','])

    def test_mismatched_and_unterminated_expressions_fail(self):
        for source in ['([)]', '("p.', '(p', "('x')"]:
            with self.subTest(source=source), self.assertRaises(ValueError):
                UPSTREAM.balanced(source, 0)

    def test_partial_process_output_stays_in_failure_evidence(self):
        failure = RUN.CAPTURE.ProcessFailure('output byte limit', 'partial', 'warning', -9)
        with patch.object(RUN.CAPTURE, 'bounded', side_effect=failure):
            completed, record = RUN.execute(['solver'])
        self.assertIsNone(completed)
        self.assertEqual(record['stdout'], 'partial')
        self.assertEqual(record['stderr'], 'warning')
        self.assertEqual(record['returncode'], -9)
        self.assertEqual(record['stop'], 'output byte limit')


if __name__ == '__main__':
    unittest.main()
