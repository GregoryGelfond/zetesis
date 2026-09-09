{selected}.
p(f(1)) :- q.
q :- not not p(f(_)):#true.
#max { (a,):a; (b,):b } = (b,) :- q.
:- selected, not q.
#maximize { 1@3,selected:selected }.
#show q/0.
