h39967
s 00000/00000/00000
d R 1.2 01/10/20 13:35:23 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTBiff.H
c Name history : 1 0 src/game/BTBiff.H
e
s 00046/00000/00000
d D 1.1 01/10/20 13:35:22 bmc 1 0
c date and time created 01/10/20 13:35:22 by bmc
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
/*    FILE: BTBiff.H                                            */
/*    ASSN:                                                     */
/*    DATE: Sat Apr 30 21:12:37 1994                            */
/****************************************************************/

#ifndef BTBIFF_H
#define BTBIFF_H

#include "BTConfig.H"

#include <Xm/MainW.h>
#include <Xm/DialogS.h>

#include <stdio.h>
#include <iostream.h>

#include "DevAudio.H"
#include "BTWidget.H"
#include "BTDrawingAreaWidget.H"
#include "BTBiff.H"

class BTPixmap;

class BTBiff {
private:
  DevAudio *dev_;
  BTPixmap *sleep_image_, *chal_image_;
  BTWidget *shell_;
  int sleep_;

public:
  BTBiff(BTWidget *parent, BTPixmap *sleep, BTPixmap *chal, DevAudio *dev);
  Widget getWidget() { return sleep_daw_->getWidget(); }
  void changeBiff();
  int asleep() { return sleep_; }
  ~BTBiff();
  void show() { sleep_daw_->manage(); }
  void hide() { sleep_daw_->unmanage(); }
  BTWIDGET_CALLBACK (BTBiff, handleExpose)
  BTDrawingAreaWidget *sleep_daw_, *chal_daw_;
};

#endif
E 1
