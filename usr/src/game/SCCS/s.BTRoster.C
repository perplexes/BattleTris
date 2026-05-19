h35764
s 00003/00003/00298
d D 1.2 01/10/21 19:25:11 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:31 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTRoster.C
c Name history : 1 0 src/game/BTRoster.C
e
s 00301/00000/00000
d D 1.1 01/10/20 13:35:30 bmc 1 0
c date and time created 01/10/20 13:35:30 by bmc
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
/*    FILE: BTRoster.C                                          */
/*    DATE: Wed Apr 20 17:56:27 1994                            */
/****************************************************************/

#include "BTConfig.H"

#include <stdio.h>

#include "BTNetManager.H"
#include "BTPlayer.H"
#include "BTPlayerRecord.H"
#include "BTRoster.H"
#include "BTDisplay.H"
#include "BTPixmap.H"

#define BT_ROSTER_WIDTH 640
#define BT_ROSTER_HEIGHT 600
#define BT_ROSTER_FRAC_BASE 600
#define BT_ROSTER_XFRAC 0.75
#define BT_ROSTER_YFRAC 0.75

#define BT_ROSTER_INFO_WIDTH 35

#define BT_ROSTER_TITLE_X1 ((int) (20 * BT_ROSTER_XFRAC))
#define BT_ROSTER_TITLE_Y1 ((int) (50 * BT_ROSTER_YFRAC))
#define BT_ROSTER_TITLE_X2 ((int) (300 * BT_ROSTER_XFRAC))
#define BT_ROSTER_TITLE_Y2 ((int) (110 * BT_ROSTER_YFRAC))

#define BT_ROSTER_IMAGE_X1 ((int) (100 * BT_ROSTER_XFRAC))
#define BT_ROSTER_IMAGE_Y1 ((int) (5 * BT_ROSTER_YFRAC))

#define BT_ROSTER_USER_LIST_X1 ((int) (20 * BT_ROSTER_XFRAC))
#define BT_ROSTER_USER_LIST_Y1 ((int) (165 * BT_ROSTER_YFRAC))
#define BT_ROSTER_USER_LIST_X2 ((int) (300 * BT_ROSTER_XFRAC))
#define BT_ROSTER_USER_LIST_Y2 ((int) (730 * BT_ROSTER_YFRAC))

#define BT_ROSTER_USER_INFO1_X1 ((int) (340 * BT_ROSTER_XFRAC))
#define BT_ROSTER_USER_INFO1_Y1 ((int) (165 * BT_ROSTER_YFRAC))
#define BT_ROSTER_USER_INFO1_X2 ((int) (780 * BT_ROSTER_XFRAC))
#define BT_ROSTER_USER_INFO1_Y2 ((int) (440 * BT_ROSTER_YFRAC))

#define BT_ROSTER_USER_INFO2_X1 ((int) (340 * BT_ROSTER_XFRAC))
#define BT_ROSTER_USER_INFO2_Y1 ((int) (455 * BT_ROSTER_YFRAC))
#define BT_ROSTER_USER_INFO2_X2 ((int) (780 * BT_ROSTER_XFRAC))
#define BT_ROSTER_USER_INFO2_Y2 ((int) (730 * BT_ROSTER_YFRAC))

#define BT_ROSTER_HEADTOHEAD_X1 ((int) (340 * BT_ROSTER_XFRAC))
#define BT_ROSTER_HEADTOHEAD_Y1 ((int) (20 * BT_ROSTER_YFRAC))
#define BT_ROSTER_HEADTOHEAD_X2 ((int) (430 * BT_ROSTER_XFRAC))
#define BT_ROSTER_HEADTOHEAD_Y2 ((int) (160 * BT_ROSTER_YFRAC))

#define BT_ROSTER_PLAYER1NAME_X1 ((int) (430 * BT_ROSTER_XFRAC))
#define BT_ROSTER_PLAYER1NAME_Y1 ((int) BT_ROSTER_HEADTOHEAD_Y1)
#define BT_ROSTER_PLAYER1NAME_X2 ((int) (605 * BT_ROSTER_XFRAC))
#define BT_ROSTER_PLAYER1NAME_Y2 ((int) BT_ROSTER_HEADTOHEAD_Y1 +\
				  (BT_ROSTER_HEADTOHEAD_Y2 - \
				   BT_ROSTER_HEADTOHEAD_Y1) / 2)

#define BT_ROSTER_PLAYER2NAME_X1 ((int) (605 * BT_ROSTER_XFRAC))
#define BT_ROSTER_PLAYER2NAME_Y1 ((int) BT_ROSTER_HEADTOHEAD_Y1)
#define BT_ROSTER_PLAYER2NAME_X2 ((int) (780 * BT_ROSTER_XFRAC))
#define BT_ROSTER_PLAYER2NAME_Y2 ((int) BT_ROSTER_HEADTOHEAD_Y1 +\
				  (BT_ROSTER_HEADTOHEAD_Y2 - \
				   BT_ROSTER_HEADTOHEAD_Y1) / 2)

#define BT_ROSTER_PLAYER1SCORE_X1 ((int) (430 * BT_ROSTER_XFRAC))
#define BT_ROSTER_PLAYER1SCORE_Y1 ((int) BT_ROSTER_HEADTOHEAD_Y1 +\
				   (BT_ROSTER_HEADTOHEAD_Y2 - \
				    BT_ROSTER_HEADTOHEAD_Y1) / 2)
#define BT_ROSTER_PLAYER1SCORE_X2 ((int) (605 * BT_ROSTER_XFRAC))
#define BT_ROSTER_PLAYER1SCORE_Y2 ((int) BT_ROSTER_HEADTOHEAD_Y2)

#define BT_ROSTER_PLAYER2SCORE_X1 ((int) (605 * BT_ROSTER_XFRAC))
#define BT_ROSTER_PLAYER2SCORE_Y1 ((int) BT_ROSTER_HEADTOHEAD_Y1 +\
				   (BT_ROSTER_HEADTOHEAD_Y2 - \
				    BT_ROSTER_HEADTOHEAD_Y1) / 2)
#define BT_ROSTER_PLAYER2SCORE_X2 ((int) (780 * BT_ROSTER_XFRAC))
#define BT_ROSTER_PLAYER2SCORE_Y2 ((int) BT_ROSTER_HEADTOHEAD_Y2)

#define BT_ROSTER_NAME_BUTTON_X1 ((int) (30 * BT_ROSTER_XFRAC))
#define BT_ROSTER_NAME_BUTTON_Y1 ((int) (740 * BT_ROSTER_YFRAC))
#define BT_ROSTER_NAME_BUTTON_X2 ((int) (150 * BT_ROSTER_XFRAC))
#define BT_ROSTER_NAME_BUTTON_Y2 ((int) (780 * BT_ROSTER_YFRAC))

#define BT_ROSTER_RANK_BUTTON_X1 ((int) (170 * BT_ROSTER_XFRAC))
#define BT_ROSTER_RANK_BUTTON_Y1 ((int) (740 * BT_ROSTER_YFRAC))
#define BT_ROSTER_RANK_BUTTON_X2 ((int) (290 * BT_ROSTER_XFRAC))
#define BT_ROSTER_RANK_BUTTON_Y2 ((int) (780 * BT_ROSTER_YFRAC))

#define BT_ROSTER_DONE_BUTTON_X1 ((int) (485 * BT_ROSTER_XFRAC))
#define BT_ROSTER_DONE_BUTTON_Y1 ((int) (740 * BT_ROSTER_YFRAC))
#define BT_ROSTER_DONE_BUTTON_X2 ((int) (635 * BT_ROSTER_XFRAC))
#define BT_ROSTER_DONE_BUTTON_Y2 ((int) (790 * BT_ROSTER_YFRAC))

char BTRoster::labelbuf_[255];

BTRoster::BTRoster(BTWidget *parent, BTNetManager *netMgr, BTPixmap *image)
: netMgr_(netMgr), sortbyname_(0), firstitem_(1), player_1_(0), player_2_(0),
  form_(parent, "BTRoster",
	BT_ROSTER_WIDTH, BT_ROSTER_HEIGHT, BT_ROSTER_FRAC_BASE),
  title_(&form_, "title", "Roster"),
  user_info1_(&form_, "user_info1", " "),
  user_info2_(&form_, "user_info2", " "),
  user_list_(&form_, "user_list", NULL, 0),
  rank_button_(&form_, "rank_button", "By Rank"),
  name_button_(&form_, "name_button", "By Name"),
  done_button_(&form_, "done_button", "Done"),
  head_to_head_(&form_, "head_to_head", "Head\nto\nHead"),
  player1_name_(&form_, "player1_name", " "),
  player2_name_(&form_, "player2_name", " "),
  player1_score_(&form_, "player1_score", " "),
  player2_score_(&form_, "player2_score", " "),
  loading_(&form_, "loading", "Loading..."),
  image_(&form_, "image", image, image->width_, image->height_)
{
  form_.placeChild( &image_, BT_ROSTER_IMAGE_X1, BT_ROSTER_IMAGE_Y1 );
  image_.manage();

  form_.placeChild( &title_, BT_ROSTER_TITLE_X1,
		    BT_ROSTER_TITLE_Y1, BT_ROSTER_TITLE_X2, BT_ROSTER_TITLE_Y2 );
//  title_.manage();

  form_.placeChild( &loading_, 
		    BT_ROSTER_USER_LIST_X1, BT_ROSTER_USER_LIST_Y1,
		    BT_ROSTER_USER_LIST_X2, BT_ROSTER_USER_LIST_Y2 );
  loading_.alignCenter();
//  loading_.manage();

  form_.placeChild( &user_info1_,
		    BT_ROSTER_USER_INFO1_X1, BT_ROSTER_USER_INFO1_Y1,
		    BT_ROSTER_USER_INFO1_X2, BT_ROSTER_USER_INFO1_Y2 );
  user_info1_.manage();
  XtAddCallback(user_info1_.getTextWidget(), XmNmotionVerifyCallback,
		handleInfoSelect_CB, this);

  form_.placeChild( &user_info2_,
		    BT_ROSTER_USER_INFO2_X1, BT_ROSTER_USER_INFO2_Y1, 
		    BT_ROSTER_USER_INFO2_X2, BT_ROSTER_USER_INFO2_Y2);
  user_info2_.manage();
  XtAddCallback(user_info2_.getTextWidget(), XmNmotionVerifyCallback,
		handleInfoSelect_CB, this);

  form_.placeChild( &user_list_,
		    BT_ROSTER_USER_LIST_X1, BT_ROSTER_USER_LIST_Y1,
		    BT_ROSTER_USER_LIST_X2, BT_ROSTER_USER_LIST_Y2);
  user_list_.addDefActionCallback(handleUserSelection_CB, this);
  user_list_.addBrowseSelCallback(handleUserSelection_CB, this);
  
  form_.placeChild( &rank_button_,
		    BT_ROSTER_RANK_BUTTON_X1, BT_ROSTER_RANK_BUTTON_Y1,
		    BT_ROSTER_RANK_BUTTON_X2, BT_ROSTER_RANK_BUTTON_Y2);
  rank_button_.manage();
  rank_button_.addActivateCallback(handleRank_CB, this);

  form_.placeChild( &name_button_,
		    BT_ROSTER_NAME_BUTTON_X1, BT_ROSTER_NAME_BUTTON_Y1,
		    BT_ROSTER_NAME_BUTTON_X2, BT_ROSTER_NAME_BUTTON_Y2);
  name_button_.manage();
  name_button_.addActivateCallback(handleName_CB, this);

  form_.placeChild( &done_button_,
		    BT_ROSTER_DONE_BUTTON_X1, BT_ROSTER_DONE_BUTTON_Y1,
		    BT_ROSTER_DONE_BUTTON_X2, BT_ROSTER_DONE_BUTTON_Y2);
  done_button_.manage();
  // BTStartup adds the callback to this button

  form_.placeChild( &head_to_head_,
		    BT_ROSTER_HEADTOHEAD_X1, BT_ROSTER_HEADTOHEAD_Y1,
		    BT_ROSTER_HEADTOHEAD_X2, BT_ROSTER_HEADTOHEAD_Y2);
  head_to_head_.noResize();
  head_to_head_.manage();

  form_.placeChild( &player1_name_,
		    BT_ROSTER_PLAYER1NAME_X1, BT_ROSTER_PLAYER1NAME_Y1,
		    BT_ROSTER_PLAYER1NAME_X2, BT_ROSTER_PLAYER1NAME_Y2);
  player1_name_.noResize();
  player1_name_.manage();

  form_.placeChild( &player2_name_,
		    BT_ROSTER_PLAYER2NAME_X1, BT_ROSTER_PLAYER2NAME_Y1,
		    BT_ROSTER_PLAYER2NAME_X2, BT_ROSTER_PLAYER2NAME_Y2);
  player2_name_.noResize();
  player2_name_.manage();

  form_.placeChild( &player1_score_,
		    BT_ROSTER_PLAYER1SCORE_X1, BT_ROSTER_PLAYER1SCORE_Y1,
		    BT_ROSTER_PLAYER1SCORE_X2, BT_ROSTER_PLAYER1SCORE_Y2);
  player1_score_.noResize();
  player1_score_.manage();

  form_.placeChild( &player2_score_,
		    BT_ROSTER_PLAYER2SCORE_X1, BT_ROSTER_PLAYER2SCORE_Y1,
		    BT_ROSTER_PLAYER2SCORE_X2, BT_ROSTER_PLAYER2SCORE_Y2);
  player2_score_.noResize();
  player2_score_.manage();
}

void BTRoster::show()
{
  user_info2_.setText(" ");
  player1_score_.setLabel(" ");
  player2_score_.setLabel(" ");
//  DISPLAY->flush();
//  DISPLAY->handleEvents();

  netMgr_->plyupdate();
  size_ = netMgr_->plylen();
D 3
  char **players = netMgr_->plyrankbuf();
E 3
I 3
  const char **players = netMgr_->plyrankbuf();
E 3
  user_list_.setList(players, size_);

  sortbyname_ = 0;
  firstitem_ = 1;

  player_1_ = player_2_ = NULL;

  if(size_ > 0)
    user_list_.selectPos( 1, 1 );

  user_list_.manage();
//  loading_.unmanage();
  form_.manage();
}

void BTRoster::hide()
{
  form_.unmanage();
//  loading_.manage();
//  user_list_.unmanage();
}

void BTRoster::handleUserSelection()
{
  BTPlayerRecord *record;
  char *selection = user_list_.string_;

  if(selection == 0)
    return;
 
  BTPlayer *player = netMgr_->plyentry(selection);

  if(player == 0)
    return;

  if (firstitem_ == 1) {
    player_1_ = player;
    user_info1_.setText(player->formatInfo(), 1, BT_ROSTER_INFO_WIDTH);
    player1_name_.setLabel(player->key());
  } else {
    player_2_ = player;
    user_info2_.setText(player->formatInfo(), 1, BT_ROSTER_INFO_WIDTH);
    player2_name_.setLabel(player->key());
  }

  if(player_1_ && player_2_) {
    if((player_1_ != player_2_) &&
       ((record = player_1_->recordAgainst(player_2_)) != 0)) {

      sprintf(labelbuf_, "%lu", record->wins_);
      player1_score_.setLabel(labelbuf_);
      sprintf(labelbuf_, "%lu", record->losses_);
      player2_score_.setLabel(labelbuf_);

    } else {
      player1_score_.setLabel(" ");
      player2_score_.setLabel(" ");
    }
  }
}

void BTRoster::handleName()
{
  if(!sortbyname_) {
D 3
    char **players = netMgr_->plynamebuf();
E 3
I 3
    const char **players = netMgr_->plynamebuf();
E 3
    user_list_.unmanage();
    user_list_.setList(players, size_);
    sortbyname_ = 1;
    user_list_.manage();
  }
}

void BTRoster::handleRank()
{
  if(sortbyname_) {
D 3
    char **players = netMgr_->plyrankbuf();
E 3
I 3
    const char **players = netMgr_->plyrankbuf();
E 3
    user_list_.unmanage();
    user_list_.setList(players, size_);
    sortbyname_ = 0;
    user_list_.manage();
  }
}
	
void BTRoster::handleInfoSelect(Widget widget, void *, void *)
{
  if (widget == user_info1_.getTextWidget())
    firstitem_ = 1;
  else
    firstitem_ = 0;
}
E 1
