edge(1,2).
edge(2,3).
{ selected(1) }.
reach(1) :- selected(1).
reach(Y) :- reach(X), edge(X,Y).
#show.
#show Y : reach(Y).
