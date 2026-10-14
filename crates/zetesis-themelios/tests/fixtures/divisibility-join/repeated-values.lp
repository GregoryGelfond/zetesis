row(3,1;3,2;4,1;6,1;6,2;7,1;9,1;12,1).
divisor(3,1;3,2).
divisible(X,T,K) :- row(X,T), divisor(D,K), X\D=0.
