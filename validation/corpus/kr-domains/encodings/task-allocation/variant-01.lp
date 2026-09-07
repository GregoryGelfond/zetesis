% Task allocation.
%
% Assign exactly one compatible agent to each task, minimizing total assignment
% cost.
%
% Assumed inputs (supplied by a separate instance file):
%   task(T)         "T is a task."
%   agent(A)        "A is an agent."
%   cost_of(A,T,C)  "C is the cost of assigning agent A to task T."

#defined task/1.
#defined agent/1.
#defined cost_of/3.

% Domain encoding

% assigned_to(A,T): agent A is assigned to task T.
% Each task gets exactly one compatible agent.
1{ assigned_to(A,T) : agent(A), compatible_with(A,T) }1 :- task(T).

% Solver directives

% Minimize the total assignment cost.
#minimize{ C,A,T : assigned_to(A,T), cost_of(A,T,C) }.

#show assigned_to/2.
% assigned_cost(A,T,C): the cost actually incurred — the cost table's entry for an
% assignment that was in fact made. This is the relation the objective above sums
% over; naming it makes it observable without also exposing the whole input table.
assigned_cost(A,T,C) :- assigned_to(A,T), cost_of(A,T,C).

#show assigned_cost/3.

% Auxiliary atoms

% compatible_with(A,T): agent A can take task T (a cost is defined for the pair).
compatible_with(A,T) :- cost_of(A,T,_).
