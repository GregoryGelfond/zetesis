% Scenario 01: A single two-task group forces one agent on both.
% @expect sat
% @cost { 6 }
% @count optimal 2
%
% Ungrouped, each task takes its cheapest agent: t1 -> a1 (1), t2 -> a2 (1), total
% 2. in_group(t1,g1) and in_group(t2,g1) force both onto one agent; either choice
% costs the same (a1: 1 + 5, or a2: 5 + 1), so grouping raises the total from 2 to
% 6 with two co-optimal assignments.

#include "../../../encodings/task-allocation/variant-03.lp".

task(t1; t2).
agent(a1; a2).

cost_of(
    a1,t1,1;
    a2,t1,5;
    a1,t2,5;
    a2,t2,1
).

in_group(
    t1,g1;
    t2,g1
).
