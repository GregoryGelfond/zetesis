data(1..2).
1{pick(X):data(X)}1.
#minimize{2*X@1,key(X):pick(X)}.
#maximize{-2*X@1,key(X):pick(X)}.
#show.
#show chosen(X+10):pick(X).
