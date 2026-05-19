h41343
s 00000/00000/00000
d R 1.2 01/10/20 13:35:32 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTScoreManager.H
c Name history : 1 0 src/game/BTScoreManager.H
e
s 00042/00000/00000
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
/*    FILE: BTScoreManager.H                                    */
/*    ASSN:                                                     */
/*    DATE: Sun Feb 20 21:29:28 1994                            */
/****************************************************************/

#ifndef BTSCOREMANAGER_H
#define BTSCOREMANAGER_H

#include "BTConfig.H"
#include "BTScore.H"
#include "BTRingNode.H"
#include "BTLine.H"
#include "BTWeaponManager.H"
#include "BTRecon.H"
#include "BTTextWidget.H"

class BTScoreManager : public BTRingNode {
friend class BTGame;
friend class BTComputer;
private:
  long max_funds_;
  long keating_; 
  long max_op_funds_;
  int need_update_;
  BTScore rep_;
  int spy_on_, tax_on_, lines_til_baz_;
  BTRecon *recon_;
  BTTextWidget *widget_rep_;
  BTWeaponManager *weapon_manager_;

public:
  BTScoreManager(BTWeaponManager *weapon,BTRecon *recon, BTWidget *parent, int x, int y);
  void receive (BTRingPacket *packet);
  void update();
  void updateDisplay();
  ~BTScoreManager();
};

#endif
E 1
