h22172
s 00000/00000/00000
d R 1.2 01/10/20 13:35:22 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTArsenal.H
c Name history : 1 0 src/game/BTArsenal.H
e
s 00033/00000/00000
d D 1.1 01/10/20 13:35:21 bmc 1 0
c date and time created 01/10/20 13:35:21 by bmc
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
/*    FILE: BTArsenal.H                                         */
/*    ASSN:                                                     */
/*    DATE: Fri Apr 29 21:55:55 1994                            */
/****************************************************************/

#ifndef BTARSENAL_H
#define BTARSENAL_H

#include "BTConfig.H"
#include "BTConstants.H"
#include "BTWeapon.H"

class BTArsenal {
friend class BTCommManager;
friend class BTWeaponManager;
private:
  BTWeapon *rep_[BT_ARSENAL_SIZE];
  short quantity_[BT_ARSENAL_SIZE];

public:
  BTArsenal();
  BTWeapon *operator[] (int index) { return rep_[index]; }
  char *getName (int index);
  int getQuantity (int index) { return quantity_[index]; }
  int buyWeapon (BTWeapon *data);
  void useWeapon (int index);
  void clear();
};
    
#endif
E 1
