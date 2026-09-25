% Task allocation with a makespan tie-breaker.
%
% Assign exactly one compatible agent to each task, minimizing total assignment
% cost, with the makespan (the latest agent completion time) as a tie-breaker.
%
% Assumed inputs (supplied by a separate instance file):
%   task(T)                 "T is a task."
%   agent(A)                "A is an agent."
%   cost_of(A,T,C)          "C is the cost of assigning agent A to task T."
%   completion_time(A,T,D)  "D is how long agent A takes to complete task T."

#defined task/1.
#defined agent/1.
#defined cost_of/3.
#defined completion_time/3.

% Domain encoding

% assigned_to(A,T): agent A is assigned to task T.
% Each task gets exactly one compatible agent.
1{ assigned_to(A,T) : agent(A), compatible_with(A,T) }1 :- task(T).

% Solver directives

% Lexicographic optimization: total cost is primary, makespan breaks ties.
#minimize{ C@2,A,T : assigned_to(A,T), cost_of(A,T,C) }.
#minimize{ M@1 : makespan(M) }.

#show assigned_to/2.
% assigned_cost(A,T,C): the cost actually incurred — the cost table's entry for an
% assignment that was in fact made. This is the relation the objective above sums
% over; naming it makes it observable without also exposing the whole input table.
assigned_cost(A,T,C) :- assigned_to(A,T), cost_of(A,T,C).

#show assigned_cost/3.

% Auxiliary atoms

% compatible_with(A,T): agent A can take task T (a cost is defined for the pair).
compatible_with(A,T) :- cost_of(A,T,_).

% agent_completion(A,Total): Total is the time agent A spends on all its assigned
% tasks (the sum of completion times; an idle agent gets 0 from the empty sum).
agent_completion(A,Total) :-
    agent(A),
    Total = #sum{ D,T : assigned_to(A,T), completion_time(A,T,D) }.

% makespan(M): M is the latest agent completion time (the max over all agents).
makespan(M) :- M = #max{ Total,A : agent_completion(A,Total) }.
