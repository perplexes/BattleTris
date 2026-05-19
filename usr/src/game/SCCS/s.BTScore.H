h51319
s 00000/00000/00000
d R 1.2 01/10/20 13:35:32 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTScore.H
c Name history : 1 0 src/game/BTScore.H
e
s 00049/00000/00000
d D 1.1 01/10/20 13:35:31 bmc 1 0
c date and time created 01/10/20 13:35:31 by bmc
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
/*    FILE: BTScore.H                                           */
/*    ASSN:                                                     */
/*    DATE: Sun Feb 20 21:09:23 1994                            */
/****************************************************************/

#ifndef BTSCORE_HEADER
#define BTSCORE_HEADER

#include "BTConfig.H"

#include <iostream.h>

class BTScore {
friend ostream& operator<< (ostream& s, BTScore &score);
 
public:
  unsigned long score_, op_score_; 
  unsigned long lines_, op_lines_;
  long funds_, op_funds_;

public:
  BTScore()
  : score_ (0), op_score_ (0), lines_ (0), op_lines_ (0),
    funds_(0), op_funds_ (0) {}
  ~BTScore() {}

  char *writebuf(char *bufptr);
  char *readbuf(char *bufptr);

  void clear() { 
    score_ = 0; op_score_ = 0; lines_ = 0; op_lines_ = 0; 
    funds_ = 0; op_funds_ = 0;
  }
};  

inline ostream& operator<< (ostream& s, BTScore& score)
{
  s << "Current score     : " << score.score_ << endl;
  s << "Opponent's score  : " << score.op_score_ << endl;
  s << "Current lines     : " << score.lines_ << endl;
  s << "Opponent's lines  : " << score.op_lines_ << endl;
  s << "Funds             : " << score.funds_ << endl << endl;
  return s;
}
    
#endif
E 1
