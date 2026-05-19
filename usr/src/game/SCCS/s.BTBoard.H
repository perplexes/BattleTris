h29835
s 00000/00000/00000
d R 1.2 01/10/20 13:35:24 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTBoard.H
c Name history : 1 0 src/game/BTBoard.H
e
s 00037/00000/00000
d D 1.1 01/10/20 13:35:23 bmc 1 0
c date and time created 01/10/20 13:35:23 by bmc
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
/*    FILE: BTBoard.H                                           */
/*    ASSN:                                                     */
/*    DATE: Tue Feb  8 21:21:01 1994                            */
/****************************************************************/

#ifndef BTBOARD_H
#define BTBOARD_H

#include "BTConfig.H"

#include <iostream.h>

#include "Block.H"
#include "BTProtocol.H"

class BTBoardManager;

class BTBoard { 
friend class BTCommManager;
friend class BTRecon;
friend class BTBoardManager;
friend ostream& operator<< (ostream &s, BTBoard &board);
private:
  BTWeaponToken motivation_;
  int height_, width_;
  Block < int > rep_;
public:
  BTBoard (BTBoardManager *board, int upsidedown = 0);
  BTWeaponToken motivation() { return motivation_; }
  void motivation (BTWeaponToken motive) { motivation_ = motive; }
  BTBoard (void) : height_(0), width_(0) {}
};
  
#endif
E 1
