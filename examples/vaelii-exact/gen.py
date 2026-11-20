#!/usr/bin/env python3
"""Exact Vaelii ASP test programs, transcribed from vaelii/vaelii.

Source: https://github.com/vaelii/vaelii, test/vaelii/asp_aspif_test.clj at
commit 03c728ce8a66c8238efc58fbd3d972a18244bd00. Each program is the statement
vector a test hands to clasp/clingo as ASPIF, written as the equivalent ground
program: the shown name for a shown atom, x<id> for an unshown one. The vertex
cover reproduces the test's java.util.Random(7) draws exactly.
"""
import os

HERE = os.path.dirname(os.path.abspath(__file__))
MASK = (1 << 48) - 1


class JavaRandom:
    def __init__(self, seed):
        self.seed = (seed ^ 0x5DEECE66D) & MASK

    def next(self, bits):
        self.seed = (self.seed * 0x5DEECE66D + 0xB) & MASK
        value = self.seed >> (48 - bits)
        return value - (1 << bits) if value >= 1 << (bits - 1) and bits == 32 else value

    def next_int(self, bound):
        if bound & -bound == bound:
            return (bound * self.next(31)) >> 31
        while True:
            bits = self.next(31)
            value = bits % bound
            if bits - value + (bound - 1) < 1 << 31:
                return value

    def next_double(self):
        return ((self.next(26) << 27) + self.next(27)) * (1.0 / (1 << 53))


HEADER = "% Exact Vaelii test program: {test}\n% vaelii/vaelii {path} @ 03c728ce8a66\n"


def write(name, test, body):
    text = HEADER.format(test=test, path="test/vaelii/asp_aspif_test.clj") + body
    with open(os.path.join(HERE, name), "w") as handle:
        handle.write(text)


def pigeonhole(n):
    holes = range(1, n)
    ident = lambda p, h: (p - 1) * (n - 1) + h
    out = []
    for p in range(1, n + 1):
        for h in holes:
            out.append("{ x%d }." % ident(p, h))
    for p in range(1, n + 1):
        out.append(":- " + ", ".join("not x%d" % ident(p, h) for h in holes) + ".")
    for h in holes:
        for p1 in range(1, n + 1):
            for p2 in range(p1 + 1, n + 1):
                out.append(":- x%d, x%d." % (ident(p1, h), ident(p2, h)))
    return "\n".join(out) + "\n"


def weighted_cover(n):
    rnd = JavaRandom(7)
    ids = range(1, n + 1)
    weights = [rnd.next_int(9) + 1 for _ in ids]
    edges = [(i, j) for i in ids for j in range(i + 1, n + 1) if rnd.next_double() < 0.12]
    out = ["{ c%d }." % i for i in ids]
    out += [":- not c%d, not c%d." % edge for edge in edges]
    out.append("#minimize { " + "; ".join("%d@0,%d : c%d" % (weights[i - 1], i, i) for i in ids) + " }.")
    return "\n".join(out) + "\n"


def main():
    write("fact.lp", "a-fact-appears-in-the-answer-set", "a.\n")
    write("unsat.lp", "an-unsatisfiable-program-reports-unsat-rather-than-throwing", "x1.\n:- x1.\n")
    write("free-choices.lp", "agreement-programs \"two free choices\"", "{ a }.\n{ b }.\n")
    write("integrity.lp", "an-integrity-constraint-forces-the-other-branch",
          "{ a }.\n{ b }.\n:- a, b.\n:- not a, not b.\n#minimize { 1@1,1 : a }.\n")
    write("two-level.lp", "higher-minimize-priority-dominates",
          "{ a }.\n{ b }.\n:- a, b.\n:- not a, not b.\n#minimize { 1@5,1 : a }.\n#minimize { 1@1,2 : b }.\n")
    write("weight-constraint.lp", "a-weight-constraint-bounds-a-cardinality",
          "{ a }.\n{ b }.\n{ c }.\n:- 2 #sum { 1,1 : a; 1,2 : b; 1,3 : c }.\n"
          ":- 3 #sum { 1,1 : not a; 1,2 : not b; 1,3 : not c }.\n#minimize { 1@1,1 : a; 1@1,2 : b }.\n")
    for n in (6, 9, 13):
        write("pigeonhole%d.lp" % n, "pigeonhole %d" % n, pigeonhole(n))
    for n in (60, 90):
        write("cover%d.lp" % n, "weighted-cover %d" % n, weighted_cover(n))


if __name__ == "__main__":
    main()
