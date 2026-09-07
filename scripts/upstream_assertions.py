"""Extract a bounded catalogue of literal-only upstream solve assertions.

This is deliberately not a C++ parser. Unsupported syntax fails or is recorded
as excluded; the original assertion and byte span remain the authority.
"""
import ast
import hashlib
import json
from pathlib import Path
import re

from comparison_contract import split_atoms

STRING = re.compile(r'"(?:[^"\\]|\\.)*"', re.S)
TRIVIA = re.compile(r'\s+|//[^\n]*|/\*.*?\*/', re.S)

def skip(text, offset):
    while match := TRIVIA.match(text, offset):
        offset = match.end()
    return offset

def literal_sequence(text):
    offset, result = 0, []
    while (offset := skip(text, offset)) < len(text):
        match = STRING.match(text, offset)
        if not match:
            raise ValueError('not adjacent ordinary string literals')
        token = match.group()
        if re.search(r'\\(?![\\"nrt])', token):
            raise ValueError('C++ escape outside the declared decoder subset')
        result.append(ast.literal_eval(token))
        offset = match.end()
    if not result:
        raise ValueError('empty literal sequence')
    return ''.join(result)

def balanced(text, opening):
    stack, offset, separators = [], opening, []
    closing = {'(': ')', '[': ']', '{': '}'}
    while offset < len(text):
        offset = skip(text, offset)
        if offset >= len(text):
            break
        if text[offset] == '"':
            match = STRING.match(text, offset)
            if not match:
                raise ValueError('unterminated string')
            offset = match.end()
            continue
        char = text[offset]
        if char in closing:
            stack.append(closing[char])
        elif char in ')]}':
            if not stack or char != stack.pop():
                raise ValueError('mismatched C++ delimiter')
            if not stack:
                return offset, separators
        elif char == ',' and len(stack) == 1:
            separators.append(offset)
        elif char == "'":
            raise ValueError('character literals outside the declared subset')
        offset += 1
    raise ValueError('unterminated C++ expression')

def catalogue(root, relative):
    path = root / relative
    text = path.read_text()
    sections = list(re.finditer(r'SECTION\("([^"\\]+)"\)', text))
    counts, rows, excluded = {}, [], []
    for start in re.finditer(r'\bREQUIRE\s*\(', text):
        opening = text.index('(', start.start())
        end, _ = balanced(text, opening)
        assertion = text[start.start():end + 1]
        section = next((m.group(1) for m in reversed(sections) if m.start() < start.start()), None)
        counts[section] = counts.get(section, 0) + 1
        identity = f'{path.stem}/{section}/{counts[section]:02}'
        try:
            call = re.search(r'\bsolve\s*\(', assertion)
            if call is None:
                raise ValueError('no solve helper call')
            call_opening = assertion.index('(', call.start())
            call_end, separators = balanced(assertion, call_opening)
            edges = [call_opening, *separators, call_end]
            args = [assertion[left + 1:right].strip() for left, right in zip(edges, edges[1:])]
            source = literal_sequence(args[0])
            expected = literal_sequence(assertion[assertion.index('(') + 1:assertion.index('==')])
            rows.append(dict(id=identity, source_file=relative, section=section,
                assertion_ordinal=counts[section], line_start=text.count('\n', 0, start.start()) + 1,
                line_end=text.count('\n', 0, end) + 1,
                byte_start=len(text[:start.start()].encode()), byte_end=len(text[:end+1].encode()),
                assertion=assertion, source=source, helper_arguments=args[1:],
                expected_helper_output=expected,
                source_sha256=hashlib.sha256(source.encode()).hexdigest(),
                file_sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
        except ValueError as error:
            excluded.append(dict(id=identity, reason=str(error)))
    return rows, excluded

def helper_models(output):
    # Selected assertions use ordinary atom lists with no quoted brackets. The
    # raw original expected diagnostics remain separate from CLI diagnostics.
    if '"' in output:
        raise ValueError('quoted helper output outside selected subset')
    assert output.startswith('([')
    depth, end = 0, None
    for index, char in enumerate(output[1:], 1):
        if char == '[':
            depth += 1
        elif char == ']':
            depth -= 1
            if depth == 0:
                end = index
                break
    text = output[2:end]
    if not text:
        return []
    assert re.fullmatch(r'\[[^\[\]]*\](?:,\[[^\[\]]*\])*', text), text
    return sorted(split_atoms(model, comma=True) for model in re.findall(r'\[([^\[\]]*)\]', text))


def verify_fixture(directory):
    """Reconstruct selected source bytes and metadata from preserved originals."""
    cases = [json.loads(line) for line in (directory / 'cases.jsonl').read_text().splitlines()]
    if len(cases) != 24:
        raise ValueError('the selected upstream contract requires exactly 24 cases')
    registry = {}
    for relative in sorted({case['source_file'] for case in cases}):
        rows, _ = catalogue(directory / 'originals', relative)
        registry.update((row['id'], row) for row in rows)
    identities = set()
    for case in cases:
        if case['native'] not in ('admit', 'refuse'):
            raise ValueError('unknown native admission contract')
        if case['id'] in identities:
            raise ValueError('duplicate fixture identity')
        identities.add(case['id'])
        arguments = case['helper_arguments']
        if len(arguments) > 1:
            raise ValueError('objective-bound helpers are outside the selected contract')
        filters = [''] if not arguments else json.loads('[' + arguments[0][1:-1] + ']')
        if case['filters'] != filters or case['expected_helper_models'] != helper_models(case['expected_helper_output']):
            raise ValueError('derived helper contract differs from original assertion')
        original = registry[case['id']]
        for key, value in original.items():
            if case[key] != value:
                raise ValueError(f"{case['id']}: reconstructed {key} differs")
    return cases
