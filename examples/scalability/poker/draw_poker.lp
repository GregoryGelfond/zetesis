%%========================================
%% Generation of Hold'em card combinations.

%% Cards conform to conventional 4-suite decks from 2 to ace.
suite(spades; clubs; diamonds; hearts).
value(2; 3; 4; 5; 6; 7; 8; 9; 10; 11; 12; 13; 14).

%% Total number of possible worlds in a 2 player game of "hold-em"
%% depends on: 2 of 52 for hero, 2 of 50 for villan, 3 of 48 for
%% the flop, 1 of 45 for turn, and 1 of 44 for river.

% CL-USER> (* (nCr 52 2) (nCr 50 2) (nCr 48 3) (nCr 45 1) (nCr 44 1))
% 55627620048000
% CL-USER> 

%%========================================
%% Different ways of dealing cards/hands...

%% 2598960, RF = 0.000 001 539 077 169
5 = { card(hero, V, S) : suite(S), value(V) } :-
    phase(five_card_hero_hand).

%% 133784560, RF = 0.000 032 320 620 56
7 = { card(hero, V, S) : suite(S), value(V) } :-
    phase(hero_hand).

%% Hero receive 2 pre-flop cards.
2 = { card(hero, V, S) : suite(S), value(V) } :-
    phase(pre_flop_hero).

%% Villan receive 2 distinct pre-flop cards.
2 = { card(villan, V, S) : suite(S), value(V),
        not card(hero, V, S) } :-
    phase(pre_flop_villan).

%% Hero & Villan cards go into their hands.
hand(hero, V, S) :- card(hero, V, S).
hand(villan, V, S) :- card(villan, V, S).

%% 3 distinct cards are shared in the flop.
3 = { card(flop, V, S) : suite(S), value(V),
        not card(hero, V, S),
        not card(villan, V, S) } :-
    phase(flop).

%% Cards in the flop go into Hero & Villan hands.
hand(hero, V, S) :- card(flop, V, S).
hand(villan, V, S) :- card(flop, V, S).

%% 1 distinct turn card is shared.
1 = { card(turn, V, S) : suite(S), value(V),
        not card(hero, V, S),
        not card(villan, V, S),
        not card(flop, V, S) } :-
    phase(turn).

%% Turn card goes into Hero & Villan hands.
hand(hero, V, S) :- card(turn, V, S).
hand(villan, V, S) :- card(turn, V, S).

%% 1 distinct river card is shared.
1 = { card(river, V, S) : suite(S), value(V),
        not card(hero, V, S),
        not card(villan, V, S),
        not card(flop, V, S),
        not card(turn, V, S) } :-
    phase(river). 

%% River card goes into Hero & Villan hands.
hand(hero, V, S) :- card(river, V, S).
hand(villan, V, S) :- card(river, V, S).

%% Determinng the choice of 5 instead of the flop, turn,
%% & river significantly reduces the cardinality of the
%% set of stable models...

%% *** 5 distinct turn & river cards are determined and shared. ***
5 = { card(shared, V, S) : suite(S), value(V),
        not card(hero, V, S),
        not card(villan, V, S) } :-
    phase(shared).

%% Shared cards go into Hero & Villan hands.
hand(hero, V, S) :- card(shared, V, S).
hand(villan, V, S) :- card(shared, V, S).

% #show card/3.
% #show hand/3.

%%========================================
%% Identification of hands values & kickers.

%% 1. A high card plays when neither player has a pair or
%%    better.  When both players have a high card, all 
%%    cards can be counted as a kicker.
high_card(P, HC, K) :-
    player(P),
    HC = #max{ V,S : hand(P, V, S) },
    K = #max{ OV,S : card(P, OV, S), OV != HC },
    #count{ 1,V,S : hand(P, V, S), V < K } >= 2.

high_card(P, HC, K) :-
    player(P),
    HC = #max{ V,S : hand(P, V, S) },
    HP = #max{ OV,S : card(P, OV, S), OV != HC },
    #count{ 1,V,S : hand(P, V, S), V < HP } <= 1,
    K = #min{ V,S : hand(P, V, S), V >= HP, not card(P, V, S) }.

%% 2. With a pair a kicker is used when your second card
%%    is better than the remaining cards on the board.
pair(P, V, K) :-
    player(P), value(V),
    hand(P, V, S1),
    hand(P, V, S2),
    S1 != S2,
    K = #max{ OV,S : hand(P, OV, S), OV != V }.

%% 3. A kicker plays with two pair only when you 
%%    do not make two pair with both of your hole cards.
two_pair(P, V1, V2, K) :-
    pair(P, V1, _),
    pair(P, V2, _),
    V1 < V2,
    K = #max{ V,S : hand(P, V, S), V != V1, V != V2 }.

%% 4. With a set, a kicker is used when your second card
%%    is better than the remaining cards on the board.
three_of_a_kind(P, V, K) :-
    player(P), value(V),
    hand(P, V, S1),
    hand(P, V, S2),
    hand(P, V, S3),
    S1 != S2, S1 != S3, S2 != S3,
    K = #max{ OV,S : hand(P, OV, S), OV != V }.

%% 5. There is no kicker with a straight because a straight
%%    is made using 5 cards.
straight(P, V, V+4) :-
    hand(P, V, _),
    % V != 14,
    hand(P, V+1, _),
    hand(P, V+2, _),
    hand(P, V+3, _),
    hand(P, V+4, _).

straight(P, 1, 5) :-
    hand(P, 14, _),
    hand(P, 2, _),
    hand(P, 3, _),
    hand(P, 4, _),
    hand(P, 5, _).

%% 6. Like a straight, there can be no kickers with flushes
%%    as the hand require 5 cards.
flush(P, S, TC) :-
    player(P), suite(S),
    #count{ 1,V : hand(P, V, S) } >= 5,
    TC = #max{ V : hand(P, V, S) }.

%% 7. There are no kickers with a full house because the hand
%%    consists of 5 cards.
full_house(P, PV, SV) :-
    pair(P, PV, PK),
    three_of_a_kind(P, SV, SK),
    PV != SV.

%% 8. A kicker does not play with a four of a kind (quads) 
%%    unless the four of a kind is on the board.
four_of_a_kind(P, V, K) :-
    player(P), value(V),
    4 = #count{ 1,S : hand(P, V, S) },
    K = #max{ OV,S : hand(P, OV, S), OV != V }.

%% 9. There are no kickers with a full house because the hand
%%    consists of 5 cards.
straight_flush(P, V, V+4) :-
    hand(P, V, S),
    % V != 14,
    hand(P, V+1, S),
    hand(P, V+2, S),
    hand(P, V+3, S),
    hand(P, V+4, S).

straight_flush(P, 1, 5) :-
    hand(P, 14, S),
    hand(P, 2, S),
    hand(P, 3, S),
    hand(P, 4, S),
    hand(P, 5, S).

%% 10. There are no kickers with a royal flush because the hand
%%     consists of 5 cards.
royal_flush(P) :-
    straight_flush(P, 10, 14).

%%------------------

% #show high_card/3.
% #show pair/3.
% #show two_pair/4.
% #show three_of_a_kind/3.
% #show straight/3.
% #show flush/3.
% #show full_house/3.
% #show four_of_a_kind/3.
% #show straight_flush/3.
% #show royal_flush/1.

%%========================================
%% Determining top hand values and hand winner.

hand_value(P, 10, 14, 14, na) :- royal_flush(P).
hand_value(P, 9, LC, HC, na) :- straight_flush(P, LC, HC).
hand_value(P, 8, V, V, K) :- four_of_a_kind(P, V, K).
hand_value(P, 7, LC, HC, na) :- full_house(P, LC, HC).
hand_value(P, 6, TC, TC, na) :- flush(P, _, TC).
hand_value(P, 5, LC, HC, na) :- straight(P, LC, HC).
hand_value(P, 4, V, V, K) :- three_of_a_kind(P, V, K).
hand_value(P, 3, LP, HP, K) :- two_pair(P, LP, HP, K).
hand_value(P, 2, V, V, K) :- pair(P, V, K).
hand_value(P, 1, V, V, K) :- high_card(P, V, K).

%% A 7-card poker hand can have multiple hand_value features making
%% it necessary to determine the max valvue of a hand.
top_hand(P, MHV, MLC, MHC, MK) :-
    player(P),
    MHV = #max{ V : hand_value(P, V, _, _, _) },
    MLC = #max{ V : hand_value(P, MHV, V, _, _) },
    MHC = #max{ V : hand_value(P, MHV, MLC, V, _) },
    MK = #max{ V : hand_value(P, MHV, MLC, MHC, V) }.

%% P1 defeats P2 when their top hand is higher.
defeats(P1, P2) :-
    player(P1), player(P2), P1 != P2,
    top_hand(P1, MHV1, _, _, _),
    top_hand(P2, MHV2, _, _, _),
    % Hand value is higher.
    MHV1 > MHV2.

%% P1 defeats P2 when hand values tie but top-card is higher.
defeats(P1, P2) :-
    player(P1), player(P2), P1 != P2,
    top_hand(P1, V, _, HC1, _),
    top_hand(P2, V, _, HC2, _),
    % High card is higher.
    HC1 > HC2.

%% P1 defeats P2 when hand/top values tie but low-card is better.
defeats(P1, P2) :-
    player(P1), player(P2), P1 != P2,
    top_hand(P1, V, LC1, HC, _),
    top_hand(P2, V, LC2, HC, _),
    % Low card is higher.
    LC1 > LC2.

%% P1 defeats P2 when their kicker is better.
defeats(P1, P2) :-
    player(P1), player(P2), P1 != P2,
    top_hand(P1, V, LC, HC, K1),
    top_hand(P2, V, LC, HC, K2),
    % Kicker is higher.
    K1 > K2.

%% When hand value, low card, high card & kicker match,
%% then the hand is a tie.
ties(P1, P2) :-
    player(P1), player(P2), P1 != P2,
    top_hand(P1, V, LC, HC, K),
    top_hand(P2, V, LC, HC, K).

%%------------------

#show hand/3.
% #show hand_value/5.
#show top_hand/5.
% #show defeats/2.
% #show ties/2.

% #project top_hand/5.
