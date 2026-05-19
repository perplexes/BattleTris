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
