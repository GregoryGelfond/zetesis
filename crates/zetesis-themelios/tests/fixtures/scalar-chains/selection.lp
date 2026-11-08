d(0..100).
small(1..5).
frame(1..10).
p(X,Y,F) :- d(X), K=1+1, Q=2/K, Y=Q*X, small(Y), frame(F).
