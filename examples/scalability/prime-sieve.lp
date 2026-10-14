% Declarative divisibility sieve: display every prime at most n.
% A composite has a divisor strictly between one and itself. The positive
% domain keeps the variable divisor nonzero; increasing n scales the join.

#const n = 30.

candidate(2..n).
composite(N) :- candidate(N), candidate(D), D < N, N \ D = 0.
prime(N) :- candidate(N), not composite(N).

#show prime/1.
