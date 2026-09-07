node(a).
node(b).
{selected(X)} :- node(X).
:- selected(a), selected(b).
