% Scenario 03: One agent, forced serialization.
% @expect sat
% @cost { 2 }
% @cautious optimal { assigned_to(a1,t1), assigned_to(a1,t2), assigned_cost(a1,t1,1), assigned_cost(a1,t2,1) }
%
% Only a1 exists, so both tasks land on it. Serialization forbids overlap, so t1
% and t2 run back-to-back (either order); each has duration 3 in a [0,10] window,
% so the 6 units fit easily. Cost is 2 regardless of order.

#include "../../../encodings/task-allocation/variant-04.lp".

task(t1; t2).
agent(a1).

cost_of(
    a1,t1,1;
    a1,t2,1
).

completion_time(
    a1,t1,3;
    a1,t2,3
).

time_window(
    t1,0,10;
    t2,0,10
).
