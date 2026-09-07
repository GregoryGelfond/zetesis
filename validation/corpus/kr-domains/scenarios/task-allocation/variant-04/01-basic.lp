% Scenario 01: Basic assignment with roomy time windows.
% @expect sat
% @cost { 2 }
% @cautious optimal { assigned_to(a1,t1), assigned_to(a2,t2), assigned_cost(a1,t1,1), assigned_cost(a2,t2,1) }
%
% No groups or precedence, windows wide enough for anything, and the two tasks go
% to different agents, so serialization is vacuous. Cost-min is unique: t1 -> a1
% (1), t2 -> a2 (1), total 2. The schedule is free (many start-time pairs tie), so
% only the assignment is pinned.

#include "../../../encodings/task-allocation/variant-04.lp".

task(t1; t2).
agent(a1; a2).

cost_of(
    a1,t1,1;
    a2,t1,3;
    a1,t2,3;
    a2,t2,1
).

completion_time(
    a1,t1,2;
    a2,t1,2;
    a1,t2,2;
    a2,t2,2
).

time_window(
    t1,0,10;
    t2,0,10
).
