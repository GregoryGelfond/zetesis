% Scenario 04: Group constraint conflicts with compatibility (UNSAT).
% @expect unsat
%
% t1 and t2 share a group but have disjoint compatible-agent sets: t1 only a1, t2
% only a2. The group forces a single agent on both, but none is compatible with
% both, so the group constraint fires and the program is UNSAT.

#include "../../../encodings/task-allocation/variant-03.lp".

task(t1; t2).
agent(a1; a2).

cost_of(a1,t1,1).   % only a1 is compatible with t1
cost_of(a2,t2,1).   % only a2 is compatible with t2

in_group(
    t1,g1;
    t2,g1
).
