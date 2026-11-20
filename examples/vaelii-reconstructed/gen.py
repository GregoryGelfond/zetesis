#!/usr/bin/env python3
"""Reconstructed Vaelii-shaped corpora (NOT the unpublished Vaelii harness).

DAG family per the published methodology: random recursive tree over n nodes
(node n0 is the root; every node i>0 gets a uniform parent in [0,i)) plus about
10% extra ancestor edges; edges are [child,parent]; edges ~= 1.1 * nodes.
Seeded and byte-deterministic.
"""
import random, sys

def dag(edges, seed):
    n = int(round(edges / 1.1))
    rng = random.Random(seed)
    parent = [None] + [rng.randrange(i) for i in range(1, n)]
    es = set((i, parent[i]) for i in range(1, n))
    while len(es) < edges:
        c = rng.randrange(1, n)
        # extra edge to a proper ancestor of c (keeps it a DAG)
        anc = []
        p = parent[c]
        while p is not None:
            anc.append(p); p = parent[p]
        a = rng.choice(anc)
        es.add((c, a))
    return n, sorted(es)

def facts(n, es, vals=False):
    out = [f"node(n{i})." for i in range(n)]
    out += [f"edge(n{c},n{p})." for c, p in es]
    if vals:
        out += [f"val(n{i},{i})." for i in range(n)]
    return "\n".join(out) + "\n"

def write(path, text):
    with open(path, "w") as f:
        f.write(text)

if __name__ == "__main__":
    seed = 20261117
    for edges, tag in ((10000, "10k"), (1000, "1k")):
        n, es = dag(edges, seed)
        k = n // 2
        write(f"dag{tag}-facts.lp", facts(n, es, vals=True))
        # V1 arithmetic join: edge pairs whose ids differ by more than K = m/2
        write(f"arith{tag}.lp", facts(n, es, vals=True) +
              f"arith(C,P) :- edge(C,P), val(C,CV), val(P,PV), CV > PV + {k}.\n#show arith/2.\n")
        # V2 full closure (materialize-the-entire-closure row shape)
        write(f"closure{tag}.lp", facts(n, es) +
              "anc(X,Y) :- edge(X,Y).\nanc(X,Z) :- edge(X,Y), anc(Y,Z).\n#show anc/2.\n")
        # V3 seeded single-source ancestors of the deepest leaf (first-ask row shape)
        depth = {0: 0}
        for c, p in sorted(es):
            pass
        par = {}
        for c, p in es:
            par.setdefault(c, []).append(p)
        d = [0] * n
        for i in range(1, n):
            d[i] = 1 + max(d[p] for p in par[i])
        probe = max(range(n), key=lambda i: (d[i], -i))
        write(f"firstask{tag}.lp", facts(n, es) +
              f"reach(n{probe}).\nreach(Y) :- reach(X), edge(X,Y).\n#show reach/1.\n")
        # V4 negation-as-failure complement of the seeded ancestor set
        write(f"naf{tag}.lp", facts(n, es) +
              f"reach(n{probe}).\nreach(Y) :- reach(X), edge(X,Y).\n"
              "nonanc(X) :- node(X), not reach(X).\n#show nonanc/1.\n")
        # V5 aggregation: per-node transitive-ancestor count
        write(f"agg{tag}.lp", facts(n, es) +
              "anc(X,Y) :- edge(X,Y).\nanc(X,Z) :- edge(X,Y), anc(Y,Z).\n"
              "cnt(X,N) :- node(X), N = #count{ Y : anc(X,Y) }.\n#show cnt/2.\n")
        # V6 same generation (OpenRuleBench sg shape over the DAG)
        write(f"sg{tag}.lp", facts(n, es) +
              "sg(X,Y) :- edge(X,P), edge(Y,P), X != Y.\n"
              "sg(X,Y) :- edge(X,A), sg(A,B), edge(Y,B), X != Y.\n#show sg/2.\n")
        # S1 3-coloring, non-ground encoding over the same graph, first answer
        col = facts(n, es) + ("col(X,r) | col(X,g) | col(X,b) :- node(X).\n"
              ":- edge(X,Y), col(X,C), col(Y,C).\n#show col/2.\n")
        write(f"color{tag}.lp", facts(n, es) +
              "1 { col(X,r); col(X,g); col(X,b) } 1 :- node(X).\n"
              ":- edge(X,Y), col(X,C), col(Y,C).\n#show col/2.\n")
        # S2 fully ground 3-coloring (the Vaelii row hands clasp ground ASPIF)
        g = []
        for i in range(n):
            g.append(f"1 {{ col(n{i},r); col(n{i},g); col(n{i},b) }} 1.")
        for c, p in es:
            for k2 in "rgb":
                g.append(f":- col(n{c},{k2}), col(n{p},{k2}).")
        g.append("#show col/2.")
        write(f"groundcolor{tag}.lp", "\n".join(g) + "\n")
        # O1 weak-constraint coloring: soft contradictions, optimum cost 0
        write(f"softcolor{tag}.lp", facts(n, es) +
              "1 { col(X,r); col(X,g); col(X,b) } 1 :- node(X).\n"
              ":~ edge(X,Y), col(X,C), col(Y,C). [1@1,X,Y]\n#show col/2.\n")
        # V7 stratified recursion: reachability through negation, then a
        # second recursive stratum over its complement
        write(f"stratrec{tag}.lp", facts(n, es, vals=True) +
              "blocked(X) :- val(X,V), V \\ 13 == 0, V > 0.\nreach(n0).\n"
              "reach(C) :- reach(P), edge(C,P), not blocked(C).\n"
              "far(X) :- node(X), not reach(X).\nup(X) :- far(X).\n"
              "up(P) :- up(C), edge(C,P), not reach(P).\n#show reach/1. #show up/1.\n")
        print(tag, "nodes", n, "edges", len(es), "K", k, "probe", probe, "depth", d[probe])
    write("queens12.lp",
          "#const n=12.\nq(X,Y) : Y=1..n :- X=1..n.\n" if False else
          "#const n=12.\n1 { q(X,1..n) } 1 :- X=1..n.\n"
          ":- q(X1,Y), q(X2,Y), X1 < X2.\n"
          ":- q(X1,Y1), q(X2,Y2), X1 < X2, X2-X1 = |Y2-Y1|.\n#show q/2.\n")
