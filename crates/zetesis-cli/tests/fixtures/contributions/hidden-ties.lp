{p(f(1)); p(f(2)); hidden; selected}.
marker :- not p(f(_)):#true.
alpha <= #min { alpha:a; omega:b } <= omega :- marker.
:- selected, not marker.
#maximize { 1@2,selected:selected; 0@1,hidden:hidden }.
#show a/0.
#show b/0.
