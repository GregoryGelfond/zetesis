% Correlated signed rows, repeated variables and a nondeterministic producer.
domain(1..2).
-edge(1,1). -edge(1,2). -edge(2,1). -edge(2,2).
1 { choose(X) : domain(X) } 1.
witness(X,Y) :- choose(X), -edge(X,Y).
diagonal(X) :- -edge(X,X).
#show witness/2.
