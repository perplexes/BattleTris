h09086
s 00000/00000/00000
d R 1.2 01/10/20 13:35:28 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTLine.H
c Name history : 1 0 src/game/BTLine.H
e
s 00032/00000/00000
d D 1.1 01/10/20 13:35:27 bmc 1 0
c date and time created 01/10/20 13:35:27 by bmc
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
/*    FILE: BTLine.H                                            */
/*    ASSN:                                                     */
/*    DATE: Wed Apr 13 17:01:31 1994                            */
/****************************************************************/

#ifndef BTLINE_H
#define BTLINE_H

#include "BTConfig.H"

class BTLine {
private:
  int increment_;
  
public:
  BTLine (int total = 0) : increment_ (0) {}
  void operator++ () { increment_++; }
  int operator== (int other) { 
    if (other == increment_) return 1;
    else return 0;
  }	
  int operator!() {
    if (increment_ == 0) return 1;
    else return 0;
  }
  int inc() { return increment_; }
};

#endif
E 1
