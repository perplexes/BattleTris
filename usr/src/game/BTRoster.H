/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTRoster.H                                          */
/*    DATE: Wed Apr 20 17:51:19 1994                            */
/****************************************************************/

#ifndef BTROSTER_H
#define BTROSTER_H

#define DECLARE_CALLBACK(nm,func)                               \
    void func(Widget w,void* ud,void* ed);                   \
    static void func##_CB(Widget w, void* ud, void* ed)         \
    { ((nm*)ud)->func(w,ud,ed); };
 

#include "BTConfig.H"
#include "BTWidget.H"
#include "BTScrolledTextWidget.H"
#include "BTScrolledListWidget.H"
#include "BTPushButtonWidget.H"
#include "BTFormWidget.H"
#include "BTLabelWidget.H"
#include "BTDrawingAreaWidget.H"

#define BT_ROSTER_WIDTH 640
#define BT_ROSTER_HEIGHT 600

class BTNetManager;
class BTPlayer;

class BTRoster {

friend class BTStartup;

public:
  BTRoster(BTWidget *parent, BTNetManager *netMgr, BTPixmap *image);
  ~BTRoster() {}

  void show();
  void hide();

  DECLARE_CALLBACK(BTRoster, handleInfoSelect)
  BTWIDGET_CALLBACK(BTRoster, handleUserSelection)
  BTWIDGET_CALLBACK(BTRoster, handleRank)
  BTWIDGET_CALLBACK(BTRoster, handleName)

protected:
  static char labelbuf_[255];
  int sortbyname_;
  int firstitem_;
  int size_;

  BTNetManager *netMgr_;
  BTPlayer *player_1_, *player_2_;

  // Do not change the order of the widget declarations since they
  // need to be constructed in the right order in the initializer list
  // of BTRoster::BTRoster

  BTFormWidget form_;
  BTLabelWidget title_;
  BTDrawingAreaWidget image_;
  BTScrolledTextWidget user_info1_;
  BTScrolledTextWidget user_info2_;
  BTScrolledListWidget user_list_;
  BTPushButtonWidget rank_button_;
  BTPushButtonWidget name_button_;
  BTPushButtonWidget done_button_;
  BTLabelWidget loading_;
  BTPushButtonWidget head_to_head_,
  player1_name_, player1_score_,
  player2_name_, player2_score_;
};

#endif
