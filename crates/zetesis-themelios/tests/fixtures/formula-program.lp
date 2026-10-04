d(1;2).
1 { picked(X) : d(X) } 1.
ready :- #count { X : picked(X) } = 1, not blocked.
-done :- not not ready.
#show.
#show chosen(X) : picked(X).
#minimize { X@1 : picked(X) }.
