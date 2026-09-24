% Task allocation with scheduling.
%
% Assign exactly one compatible agent to each task and schedule each task on a
% discrete time horizon, minimizing total assignment cost. Tasks in a group share
% an agent; each task respects its time window; precedence and per-agent
% serialization (no overlap on one agent) are enforced.
%
% Assumed inputs (supplied by a separate instance file):
%   task(T)                 "T is a task."
%   agent(A)                "A is an agent."
%   cost_of(A,T,C)          "C is the cost of assigning agent A to task T."
%   in_group(T,G)           "task T belongs to group G."
%   completion_time(A,T,D)  "D is how long agent A takes to complete task T."
%   time_window(T,E,L)      "task T must start no earlier than E and end no later than L."
%   before(X,Y)             "task X must complete before task Y starts."
%
% A task with no in_group/2 fact is ungrouped and may be assigned independently.

#defined task/1.
#defined agent/1.
#defined cost_of/3.
#defined in_group/2.
#defined completion_time/3.
#defined time_window/3.
#defined before/2.

% Domain encoding

% assigned_to(A,T): agent A is assigned to task T.
% Each task gets exactly one compatible agent.
1{ assigned_to(A,T) : agent(A), compatible_with(A,T) }1 :- task(T).

% Tasks sharing a group go to the same agent.
:- same_group(T1,T2), assigned_to(A1,T1), assigned_to(A2,T2), A1 != A2.

% start_time(T,S): task T starts at time S.
% Each task picks exactly one start time from the horizon.
1{ start_time(T,S) : time(S) }1 :- task(T).

% A task may not start before its window opens.
:- start_time(T,Start), time_window(T,Earliest,_), Start < Earliest.

% A task may not end after its window closes.
:- end_time(T,End), time_window(T,_,Latest), End > Latest.

% Precedence: a task that must precede another ends no later than the other starts.
:- before(T1,T2), end_time(T1,End1), start_time(T2,Start2), End1 > Start2.

% Agent serialization: two tasks on one agent may not overlap. The T1 < T2 guard
% avoids redundant firings on each unordered pair.
:- same_agent(T1,T2), T1 < T2, overlaps_with(T1,T2).

% Solver directives

% Minimize the total assignment cost.
#minimize{ C,A,T : assigned_to(A,T), cost_of(A,T,C) }.

#show assigned_to/2.
#show start_time/2.
#show end_time/2.
% assigned_cost(A,T,C): the cost actually incurred — the cost table's entry for an
% assignment that was in fact made. This is the relation the objective above sums
% over; naming it makes it observable without also exposing the whole input table.
assigned_cost(A,T,C) :- assigned_to(A,T), cost_of(A,T,C).

#show assigned_cost/3.

% Auxiliary atoms

% compatible_with(A,T): agent A can take task T (a cost is defined for the pair).
compatible_with(A,T) :- cost_of(A,T,_).

% horizon(M) is the latest deadline in any window; time(S) enumerates the integer
% points [0, M] that start_time/2 draws from.
horizon(M) :- M = #max{ L : time_window(_,_,L) }.

time(0..M) :- horizon(M).

% end_time(T,E): task T ends at time E, its start plus the assigned agent's
% completion time.
end_time(T,End) :-
    start_time(T,Start),
    assigned_to(A,T),
    completion_time(A,T,D),
    End = Start + D.

% same_agent(T1,T2): T1 and T2 are assigned to the same agent.
same_agent(T1,T2) :- assigned_to(A,T1), assigned_to(A,T2).

% same_group(T1,T2): T1 and T2 share at least one group.
same_group(T1,T2) :- in_group(T1,G), in_group(T2,G).

% overlaps_with(T1,T2): the tasks' time intervals overlap (each starts strictly
% before the other ends).
overlaps_with(T1,T2) :-
    start_time(T1,Start1),
    end_time(T1,End1),
    start_time(T2,Start2),
    end_time(T2,End2),
    Start1 < End2,
    Start2 < End1.
