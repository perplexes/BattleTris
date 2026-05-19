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
