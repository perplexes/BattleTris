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

BTBiff::BTBiff(BTWidget *parent, BTPixmap *sleep, BTPixmap *chal, DevAudio *dev) 
: sleep_image_ (sleep), chal_image_ (chal), dev_ (dev)
{
	Arg args[20];
	int n = 0;
	int i = 0;
 
	Visual *visual;
	Pixmap bg_pixmap;
	Pixmap brdr_pixmap;
	Colormap colormap;
	int depth;

	if (sleep)
		sleep->ref();
	if (chal)
		chal->ref();

	XtVaGetValues (parent->getWidget(), 
	    XmNvisual, &visual,
	    XmNbackgroundPixmap, &bg_pixmap,
	    XmNborderPixmap, &brdr_pixmap,
	    XmNcolormap, &colormap,
	    XmNdepth, &depth,
	    0);

	XtSetArg(args[n], XmNallowShellResize, True); n++;
	XtSetArg(args[n], XmNvisual, visual); n++;
	XtSetArg(args[n], XmNbackgroundPixmap, bg_pixmap); n++;
	XtSetArg(args[n], XmNborderPixmap, brdr_pixmap); n++;
	XtSetArg(args[n], XmNcolormap, colormap); n++;
	XtSetArg(args[n], XmNdepth, depth); n++;
	shell_ = new BTWidget(parent,
	    XmCreateDialogShell(parent->getWidget(), (char *)"biff", args, n));

	sleep_daw_ = new BTDrawingAreaWidget (shell_, "biff_daw", sleep_image_, 
	    sleep_image_->width_, sleep_image_->height_, 0, 0);

	sleep_daw_->setShape((char *)btsleepmask_bits, btsleepmask_width,
	    btsleepmask_height);

	sleep_ = 1;
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
