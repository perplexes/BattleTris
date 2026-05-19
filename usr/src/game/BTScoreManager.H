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
