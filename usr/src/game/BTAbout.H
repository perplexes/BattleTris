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
