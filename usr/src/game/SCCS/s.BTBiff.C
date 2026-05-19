h11417
s 00036/00035/00060
d D 1.2 01/10/21 19:25:05 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:23 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTBiff.C
c Name history : 1 0 src/game/BTBiff.C
e
s 00095/00000/00000
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
/*    FILE: BTBiff.C                                            */
/*    ASSN:                                                     */
/*    DATE: Sat Apr 30 21:30:14 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "DevAudio.H"
#include "BTBiff.H"
#include "BTPixmap.H"

#if HAVE_X11_EXTENSIONS_SHAPE_H
# include "../art/btsleepmask.xbm"
# include "../art/btchalmask.xbm"
#else
# define btsleepmask_bits ((char *) 0)
# define btsleepmask_width 0
# define btsleepmask_height 0
# define btchalmask_bits ((char *) 0)
# define btchalmask_width 0
# define btchalmask_height 0
#endif

#define BT_BIFF_WIDTH 200
#define BT_BIFF_HEIGHT 200

D 3
BTBiff::BTBiff (BTWidget *parent, BTPixmap *sleep, BTPixmap *chal, DevAudio *dev) 
: sleep_image_ (sleep), chal_image_ (chal), dev_ (dev) {
  Arg args[20];
  int n = 0;

  int i = 0;
E 3
I 3
BTBiff::BTBiff(BTWidget *parent, BTPixmap *sleep, BTPixmap *chal, DevAudio *dev) 
: sleep_image_ (sleep), chal_image_ (chal), dev_ (dev)
{
	Arg args[20];
	int n = 0;
	int i = 0;
E 3
 
D 3
  Visual *visual;
  Pixmap bg_pixmap;
  Pixmap brdr_pixmap;
  Colormap colormap;
  int depth;
E 3
I 3
	Visual *visual;
	Pixmap bg_pixmap;
	Pixmap brdr_pixmap;
	Colormap colormap;
	int depth;
E 3

D 3
  if ( sleep )
    sleep->ref();
  if ( chal )
    chal->ref();
E 3
I 3
	if (sleep)
		sleep->ref();
	if (chal)
		chal->ref();
E 3

D 3
  XtVaGetValues (parent->getWidget(), 
    XmNvisual, &visual,
    XmNbackgroundPixmap, &bg_pixmap,
    XmNborderPixmap, &brdr_pixmap,
    XmNcolormap, &colormap,
    XmNdepth, &depth,
    0);
E 3
I 3
	XtVaGetValues (parent->getWidget(), 
	    XmNvisual, &visual,
	    XmNbackgroundPixmap, &bg_pixmap,
	    XmNborderPixmap, &brdr_pixmap,
	    XmNcolormap, &colormap,
	    XmNdepth, &depth,
	    0);
E 3

D 3
  XtSetArg (args[n], XmNallowShellResize, True); n++;
  XtSetArg (args[n], XmNvisual, visual); n++;
  XtSetArg (args[n], XmNbackgroundPixmap, bg_pixmap); n++;
  XtSetArg (args[n], XmNborderPixmap, brdr_pixmap); n++;
  XtSetArg (args[n], XmNcolormap, colormap); n++;
  XtSetArg (args[n], XmNdepth, depth); n++;
  shell_ = new BTWidget(parent, XmCreateDialogShell (parent->getWidget(), "biff", args, n));
E 3
I 3
	XtSetArg(args[n], XmNallowShellResize, True); n++;
	XtSetArg(args[n], XmNvisual, visual); n++;
	XtSetArg(args[n], XmNbackgroundPixmap, bg_pixmap); n++;
	XtSetArg(args[n], XmNborderPixmap, brdr_pixmap); n++;
	XtSetArg(args[n], XmNcolormap, colormap); n++;
	XtSetArg(args[n], XmNdepth, depth); n++;
	shell_ = new BTWidget(parent,
	    XmCreateDialogShell(parent->getWidget(), (char *)"biff", args, n));
E 3

D 3
  sleep_daw_ = new BTDrawingAreaWidget (shell_, "biff_daw", sleep_image_, 
                                        sleep_image_->width_,
                                        sleep_image_->height_,
                                        0, 0);
E 3
I 3
	sleep_daw_ = new BTDrawingAreaWidget (shell_, "biff_daw", sleep_image_, 
	    sleep_image_->width_, sleep_image_->height_, 0, 0);
E 3

D 3
  sleep_daw_->setShape ((char *) btsleepmask_bits, btsleepmask_width, btsleepmask_height);
  sleep_ = 1;
E 3
I 3
	sleep_daw_->setShape((char *)btsleepmask_bits, btsleepmask_width,
	    btsleepmask_height);

	sleep_ = 1;
E 3
}

void BTBiff::handleExpose() {
  cerr << "In shell's expose callback..." << endl;
}

void BTBiff::changeBiff() {
  if (sleep_) {
    sleep_daw_->setShape ((char *) btchalmask_bits, btchalmask_width, btchalmask_height);
    sleep_daw_->setImage(chal_image_);
    sleep_ = 0;
  } else {
    sleep_daw_->setShape ((char *) btsleepmask_bits, btsleepmask_width, btsleepmask_height);
    sleep_daw_->setImage(sleep_image_);
    sleep_ = 1;
  }
}

BTBiff::~BTBiff() {
  if (sleep_image_->deref())
    delete sleep_image_;
  if (chal_image_->deref())
    delete chal_image_;
  delete sleep_daw_;
  delete shell_;
}  
E 1
