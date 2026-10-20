% The first alternative requires two distinct contributions to the bounded row.
0 { a; b; c; d } 2.
{ x; y }.
a :- x.
b :- y.
good :- x, y.
good :- c, d.
:- not good.
