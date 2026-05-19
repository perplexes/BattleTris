h56576
s 00000/00000/00000
d R 1.2 01/10/20 13:35:26 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTChallengeDialog.H
c Name history : 1 0 src/game/BTChallengeDialog.H
e
s 00050/00000/00000
d D 1.1 01/10/20 13:35:25 bmc 1 0
c date and time created 01/10/20 13:35:25 by bmc
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
/*    FILE: BTChallengeDialog.H                                 */
/*    ASSN:                                                     */
/*    DATE: Wed May  4 17:37:33 1994                            */
/****************************************************************/

#ifndef BTCHALLENGEDIALOG_H
#define BTCHALLENGEDIALOG_H

#include "BTConfig.H"
#include "BTPushButtonWidget.H"
#include "BTFormWidget.H"
#include "BTLabelWidget.H"
#include "BTDrawingAreaWidget.H"
#include "BTScrolledTextWidget.H"

#include <Xm/Xm.h>
#include <Xm/DialogS.h>
#include <Xm/MessageB.h>

class BTPlayer;

class BTChallengeDialog : public BTWidget {
friend class BTStartup;

private:
  static void _chal_cb_ (Widget, XtPointer thisp, XtPointer);
/*
  BTPushButtonWidget *no_button_, *yes_button_;
  BTLabelWidget *headline_, *byline_;
  */
  BTPushButtonWidget *accept_, *decline_;
  BTFormWidget *form_;
  BTLabelWidget *challenger_, *chal_smack_;
  BTScrolledTextWidget *chal_info_;
  BTWidget *shell_;
  int width_, height_;
public:
  void show();
  void hide();
  void player(BTPlayer *);
  BTChallengeDialog (BTWidget *parent, char *headline = 0, char *byline = 0);
  ~BTChallengeDialog();
};



#endif
E 1
