# my_project/__init__.py
from . import *
# OR if using a specific module-name:
from .core import utils, ExecutionStrategy, Order, OrderType, OrderSide, OMSEngine, OrderState
from .client import Didius