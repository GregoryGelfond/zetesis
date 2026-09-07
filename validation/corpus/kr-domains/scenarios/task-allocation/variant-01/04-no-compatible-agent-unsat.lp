% Scenario 04: A task with no compatible agent (UNSAT).
% @expect unsat
%
% No cost_of fact mentions t2, so compatible_with(_,t2) is empty. The choice rule
% 1 { assigned_to(A,T) : agent(A), compatible_with(A,T) } 1 :- task(T) cannot be
% satisfied for t2 (zero candidates against a lower bound of 1), so the program
% is UNSAT.

#include "../../../encodings/task-allocation/variant-01.lp".

task(t1; t2).

agent(a1; a2; a3).

cost_of(
    a1,t1,3;
    a2,t1,5;
    a3,t1,4
).

% No cost_of fact for t2: t2 has no compatible agent.
