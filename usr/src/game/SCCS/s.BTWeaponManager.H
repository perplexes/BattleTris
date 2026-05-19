h51131
s 00000/00000/00000
d R 1.2 01/10/20 13:35:36 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTWeaponManager.H
c Name history : 1 0 src/game/BTWeaponManager.H
e
s 00044/00000/00000
d D 1.1 01/10/20 13:35:35 bmc 1 0
c date and time created 01/10/20 13:35:35 by bmc
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
/*    FILE: BTWeaponManager.H                                   */
/*    ASSN:                                                     */
/*    DATE: Wed Apr 13 18:40:09 1994                            */
/****************************************************************/

#ifndef BTWEAPONMANAGER_H
#define BTWEAPONMANAGER_H

#include "BTConfig.H"
#include "BTRingNode.H"
#include "BTWeapon.H"
#include "Block.H"
#include "BTLine.H"
#include "BTArsenal.H"

class BTCommManager;
class BTPushButtonWidget;
class BTTextWidget;
class BTFormWidget;

class BTWeaponManager : public BTRingNode {
friend class BTGame;
friend class BTComputer;
private:
  short remaining_[BT_MAX_WEAPONS];
  BTArsenal *arsenal_, *old_arsenal_;
  BTTextWidget *rep_;
  BTPushButtonWidget **label_; 
  BTCommManager *comm_manager_;
  int need_ars_, computer_;

public:
  short BTActive[BT_MAX_WEAPONS];
  BTWeaponManager(BTFormWidget *parent, int x, int y, BTCommManager *comm_manager, int computer = 0);
  void receive (BTRingPacket *packet);
  void update( int num = BT_ARSENAL_SIZE );
  void launchWeapon (int number);
  ~BTWeaponManager();
};

#endif
E 1
