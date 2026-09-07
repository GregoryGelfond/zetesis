% Scenario 04: A window too short for any agent's duration (UNSAT).
% @expect unsat
%
% t1's window [0,3] allows at most 3 units of duration, but every compatible agent
% takes at least 4 (a1: 5, a2: 4), so no agent and start time satisfy the window.
% The program is UNSAT.

#include "../../../encodings/task-allocation/variant-04.lp".

task(t1).
agent(a1; a2).

cost_of(
    a1,t1,1;
    a2,t1,2
).
completion_time(
    a1,t1,5;
    a2,t1,4
).

time_window(t1,0,3).
