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
