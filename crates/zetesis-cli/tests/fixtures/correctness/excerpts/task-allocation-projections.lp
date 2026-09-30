% Extracted S0 fixture for zetesis, not a complete correctness example.
% Upstream MIT: Copyright (c) 2026 Gregory Gelfond. See ../README.md and ../LICENSE.
% Snapshot: 38f0660ded448ed268c5a68759ceb0e2840dd497.

% Synthetic scalar facts authored for this fixture.
% Expected: SAT; exactly one complete model.
cost_of(alice,t1,3). cost_of(bob,t1,5).
assigned_to(alice,t1).

% Verbatim upstream rule fragment: encodings/task-allocation/variant-01.lp:30,37.
assigned_cost(A,T,C) :- assigned_to(A,T), cost_of(A,T,C).

compatible_with(A,T) :- cost_of(A,T,_).
