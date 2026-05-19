h49457
s 00000/00000/00000
d R 1.2 01/10/20 13:35:23 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTBoard.C
c Name history : 1 0 src/game/BTBoard.C
e
s 00050/00000/00000
d D 1.1 01/10/20 13:35:22 bmc 1 0
c date and time created 01/10/20 13:35:22 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTBoard.C                                           */
/*    ASSN:                                                     */
/*    DATE: Tue Feb  8 21:58:15 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTBoard.H"
#include "BTBoardManager.H"
#include "BTBox.H"

BTBoard::BTBoard (BTBoardManager *board, int upsidedown) {
  height_ = board->height_;
  width_ = board->width_;
  rep_.resize (height_ * width_);
  for (int i = 0; i < height_; i++) {
    int new_y;
    if ( upsidedown )
      new_y = height_ - i - 1;
    else
      new_y = i;
    for (int j = 0; j < width_; j++) {
      int id;
      if (board->map_[j][i]) {
        rep_ [new_y * width_ + j] = board->map_[j][i]->id();
      } else
        rep_ [new_y * width_ + j] = 0;
/*
      if (rep_ [new_y * width_ + j] == BT_NEUTRAL) 
        rep_ [new_y * width_ + j] = (char) 0;
*/
    }
  }
}

ostream& operator<< (ostream& s, BTBoard& b) {
  for (int i = 0; i < b.height_; i++) {  
    for (int j = 0; j < b.width_; j++) {
      if (!b.rep_[i*b.width_+j])
        s << ".";
      else if (b.rep_[i * b.width_ + j] < 100) 
        s << "@";
      else s << b.rep_[i * b.width_ + j] - 100;
    }
    s << endl;
  }
  return s;
}  
E 1
