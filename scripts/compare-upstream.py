#!/usr/bin/env python3
"""Replay exact, objective-free clingo assertions with complete model contracts.

This bounded semantic campaign is not a benchmark or a language-wide percentage.
Original helper filters are checked separately from full answer-model identity.
"""
import argparse
import collections
import datetime
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shutil
import sys
import tempfile

from comparison_contract import clingo, identity, native
from upstream_assertions import verify_fixture

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('paired_capture', ROOT / 'scripts/compare-optimal.py')
CAPTURE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CAPTURE)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def report_path(report, protected, fixture_directory):
    """Reject lexical, symlink and existing hard-link aliases before execution."""
    resolved = report.resolve()
    if resolved.is_relative_to(fixture_directory.resolve()):
        raise ValueError('report must not be written inside the pinned fixture directory')
    for path in protected:
        if resolved == path.resolve() or (report.exists() and path.exists() and report.samefile(path)):
            raise ValueError(f'report aliases a protected input: {path}')


def profile_refusal(completed):
    # A generic source-admission prefix also covers resource and load failures.
    # This selected corpus expects explicit profile refusals only. The portable
    # frontend test checks the corresponding typed Profile error independently.
    return completed.returncode == 2 and re.search(
        r'^zetesis: source admission: S0 does not admit [^\n]+$',
        completed.stderr, re.MULTILINE) is not None


def execute(command):
    try:
        completed, seconds = CAPTURE.bounded(command, 5, 1_114_112)
        return completed, dict(command=command, returncode=completed.returncode,
            stdout=completed.stdout, stderr=completed.stderr, elapsed_seconds=seconds, stop=None)
    except CAPTURE.ProcessFailure as error:
        return None, dict(command=command, stop=str(error), **error.evidence)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--clingo', default='clingo')
    parser.add_argument('--zetesis', required=True)
    parser.add_argument('--report', required=True, type=Path)
    args = parser.parse_args()
    directory = ROOT / 'validation/upstream/clingo-5.8.2'
    cases = verify_fixture(directory)
    files = {str(path.relative_to(directory)): digest(path) for path in directory.rglob('*') if path.is_file()}
    binaries = {key: Path(shutil.which(value) or value).resolve(strict=True)
        for key, value in [('clingo', args.clingo), ('zetesis', args.zetesis)]}
    hashes = {key: digest(path) for key, path in binaries.items()}
    protected = [directory / name for name in files] + list(binaries.values())
    protected.extend((ROOT / 'scripts').rglob('*.py'))
    protected.append(Path(sys.executable))
    report_path(args.report, protected, directory)
    report = dict(schema=1, started_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
        upstream_commit='a99ffb2a58293c68b28fcc283a1d1c9ccad900fe',
        scope='Exact selected objective-free sources; full model multiplicities plus original prefix-filter contract.',
        binaries={key: dict(path=str(path), sha256=hashes[key]) for key, path in binaries.items()},
        input_sha256=files, limits=dict(seconds_per_process=5, captured_bytes_per_process=1_114_112,
            native='unchanged CLI defaults; workers=1; CPU; exhaustive enumeration'), cases=[])
    with tempfile.TemporaryDirectory(prefix='zetesis-upstream-') as temporary:
        for case in cases:
            row = dict(id=case['id'], source_sha256=case['source_sha256'], expected_native=case['native'])
            source = case['source']
            if '#show' in source or len(case['helper_arguments']) > 1:
                raise ValueError('selected contract excludes #show and objective-bound helper arguments')
            path = Path(temporary) / 'input.lp'
            path.write_text(source)
            reference, row['reference_run'] = execute([str(binaries['clingo']), str(path), '--outf=2', '--models=0'])
            candidate, row['native_run'] = execute([str(binaries['zetesis']), str(path), '--backend', 'cpu',
                '--workers', '1', '--models', '0', '--stats'])
            try:
                if reference is None:
                    raise ValueError('reference incomplete')
                expected = clingo(reference)
                if expected['cost'] is not None or expected['models'] != sorted(case['models']):
                    raise ValueError('reference differs from recorded full model contract')
                filtered = sorted(sorted(atom for atom in model
                    if any(atom.startswith(prefix) for prefix in case['filters'])) for model in expected['models'])
                if filtered != case['expected_helper_models']:
                    raise ValueError('reference differs from original helper-filter contract')
                row['reference_contract'] = expected
                if candidate is None:
                    row['classification'] = 'native-incomplete'
                elif profile_refusal(candidate):
                    row['classification'] = 'native-source-refusal'
                else:
                    actual = native(candidate)
                    row['native_contract'] = actual
                    row['classification'] = 'parity-pass' if identity(actual) == identity(expected) else 'semantic-mismatch'
            except (ValueError, KeyError, TypeError) as error:
                row['classification'] = 'contract-failure'
                row['reason'] = str(error)
            row['expectation_met'] = row['classification'] == (
                'parity-pass' if case['native'] == 'admit' else 'native-source-refusal')
            report['cases'].append(row)
            print(case['id'] + ': ' + row['classification'], flush=True)
    report['counts'] = dict(collections.Counter(row['classification'] for row in report['cases']))
    report['inputs_and_binaries_unchanged'] = files == {
        str(path.relative_to(directory)): digest(path) for path in directory.rglob('*') if path.is_file()
    } and all(digest(path) == hashes[key] for key, path in binaries.items())
    report['passed'] = report['inputs_and_binaries_unchanged'] and all(row['expectation_met'] for row in report['cases'])
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
