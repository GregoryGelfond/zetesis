number(2..60).
divisor(2..7).
proper_divisor(N,D) :- number(N), divisor(D), D<N, N\D=0.
#show proper_divisor/2.
