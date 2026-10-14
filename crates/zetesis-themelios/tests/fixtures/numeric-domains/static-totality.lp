candidate(2..30).
composite(N) :- candidate(N), candidate(D), D < N, N \ D = 0.
prime(N) :- candidate(N), not composite(N).
#show prime/1.
