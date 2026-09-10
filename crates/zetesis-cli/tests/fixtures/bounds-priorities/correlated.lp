row(1,2). row(2,1).
1 {a;b} 1.
selected(1) :- a.
selected(2) :- b.
#minimize {W@P,key:row(P,W),selected(P)}.
