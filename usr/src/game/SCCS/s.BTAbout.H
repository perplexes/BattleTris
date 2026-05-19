h28723
s 00000/00000/00000
d R 1.2 01/10/20 13:35:34 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTAbout.H
c Name history : 1 0 src/game/BTAbout.H
e
s 00062/00000/00000
d D 1.1 01/10/20 13:35:33 bmc 1 0
c CodeManager Uniquification: src/game/BTAbout.H
c date and time created 01/10/20 13:35:33 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTAbout.H                                           */
/*    ASGN: Final                                               */
/*    DATE: Mon May  9 20:04:48 1994                            */
/****************************************************************/

#ifndef BTABOUT_H
#define BTABOUT_H

#include "BTConfig.H"
#include "BTFormWidget.H"
#include "BTLabelWidget.H"
#include "BTDrawingAreaWidget.H"
#include "BTPushButtonWidget.H"
#include "BTTextWidget.H"

#define BT_ABOUT_WIDTH 640
#define BT_ABOUT_HEIGHT 600

#define BT_BRYAN_EGG    0
#define BT_CHARLIE_EGG  1
#define BT_MIKE_EGG     2
#define BT_LIBBY_EGG    3
#define BT_KEVIN_EGG    4
#define BT_MAX_EGGS     5

class BTPixmap;

class BTAbout {
friend class BTStartup;

public:
  BTAbout(BTWidget *, BTPixmap *);
  ~BTAbout();

  inline void show() { form_->manage(); };
  inline void hide() { form_->unmanage(); };

  static void handleLeftClick (BTWidget *, void *);
  static void handleRightClick (BTWidget *, void *);
  static void handleBryanClick (BTWidget *, void *);
  static void handleMikeClick (BTWidget *, void *);
  static void handleCharlieClick (BTWidget *, void *);
  static void handleLibbyClick (BTWidget *, void *);
  static void handleKevinClick (BTWidget *, void *);

protected:
  int click_;
  BTFormWidget *form_;
  BTDrawingAreaWidget *left_drawing_area_, *right_drawing_area_;
  BTLabelWidget *title_, *version_, *copyright_;
  BTLabelWidget *bryan_, *charlie_, *mike_;
  int eggs_[BT_MAX_EGGS];
  BTLabelWidget *beta1_, *beta2_, *beta3_;
  BTLabelWidget *thanks_[10];
  BTLabelWidget *special1_, *special2_;
  BTPushButtonWidget *ok_button_;
};

#endif
E 1
