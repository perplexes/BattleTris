h07524
s 00000/00000/00000
d R 1.2 01/10/20 13:35:30 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTPimp.H
c Name history : 1 0 src/game/BTPimp.H
e
s 00030/00000/00000
d D 1.1 01/10/20 13:35:29 bmc 1 0
c date and time created 01/10/20 13:35:29 by bmc
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
/*    FILE: BTPimp.H                                            */
/*    ASSN:                                                     */
/*    DATE: Fri Apr 29 02:09:32 1994                            */
/****************************************************************/

#ifndef BTPIMP_H
#define BTPIMP_H

#include "BTConfig.H"
#include "Block.H"
#include "BTWeapon.H"

class BTPimp {
private:
  short purchases_[BT_MAX_WEAPONS];
  BTWeapon *cathouse_[BT_MAX_WEAPONS];

public:
  BTPimp();
  ~BTPimp();
  BTWeapon *operator[] (BTWeaponToken index);
  BTWeapon *operator[] (int index);
  void purchase(BTWeaponToken index);
  int load();
};
 
#endif
E 1
