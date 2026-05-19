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
